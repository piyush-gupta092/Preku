package app.preku.bridge

import android.os.Build
import app.preku.BuildConfig
import app.preku.bridge.protocol.DeviceStatus
import app.preku.bridge.protocol.Hello
import app.preku.bridge.protocol.MessageType
import app.preku.bridge.protocol.PairRequest
import app.preku.bridge.protocol.Pong
import app.preku.data.SettingDao
import app.preku.data.SettingEntity
import app.preku.di.ApplicationScope
import app.preku.domain.Clock
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener

enum class LinkStatus { NotPaired, Pairing, Connecting, Connected, Offline }

data class BridgeUiState(
    val pc: PairedPc? = null,
    val status: LinkStatus = LinkStatus.NotPaired,
    /** Why the last attempt failed (pair.rejected reason or a network message). */
    val problem: String? = null,
)

/**
 * The phone's side of the bridge (Architecture.md §4.2): one connection to the paired PC,
 * shared by the UI and [BridgeService]. Reconnects with [Backoff]; a network change retries at
 * once, so the link is back within seconds when Wi-Fi returns.
 */
@Singleton
class BridgeManager @Inject constructor(
    private val settings: SettingDao,
    private val clock: Clock,
    /** File sharing rides on this connection. */
    val files: FileTransfers,
    @ApplicationScope private val scope: CoroutineScope,
) {
    private val _state = MutableStateFlow(BridgeUiState())
    val state: StateFlow<BridgeUiState> = _state.asStateFlow()

    /** False until the saved pairing is loaded. */
    private val _ready = MutableStateFlow(false)
    val ready: StateFlow<Boolean> = _ready.asStateFlow()

    private val backoff = Backoff()
    private var socket: WebSocket? = null
    private var retry: Job? = null
    private var running = false
    private var lastStatus: DeviceStatus? = null
    /** Bumped on every new connection, so callbacks from an old socket are ignored. */
    @Volatile private var generation = 0

    private val loaded = scope.launch {
        val pc = settings.get(KEY)?.let { runCatching { BridgeJson.decodeFromString(PairedPc.serializer(), it) }.getOrNull() }
        _state.value = BridgeUiState(pc, if (pc == null) LinkStatus.NotPaired else LinkStatus.Offline)
        _ready.value = true
    }

    /** Keep the paired PC connected (called by [BridgeService]). */
    fun start() = scope.launch {
        loaded.join()
        running = true
        if (_state.value.pc != null && socket == null) connect(null)
    }

    fun stop() {
        running = false
        retry?.cancel()
        socket?.close(1000, null)
        socket = null
        _state.update { it.copy(status = if (it.pc == null) LinkStatus.NotPaired else LinkStatus.Offline) }
    }

    /** Pair using a scanned (or pasted) QR link. */
    fun pair(link: PairingLink) = scope.launch {
        loaded.join()
        retry?.cancel()
        socket?.close(1000, null)
        _state.value = BridgeUiState(pc = null, status = LinkStatus.Pairing)
        connect(link)
    }

    fun unpair() = scope.launch {
        socket?.send(emptyEnvelopeJson(MessageType.UNPAIR))
        socket?.close(1000, null)
        forget()
    }

    /** Wi-Fi (or any network) came back: try now instead of waiting for the backoff. */
    fun onNetworkAvailable() {
        if (running && _state.value.pc != null && _state.value.status != LinkStatus.Connected) {
            backoff.reset()
            retry?.cancel()
            scope.launch { connect(null) }
        }
    }

    /** mDNS found the paired PC at a new address. */
    fun onPcFound(pcId: String, host: String, port: Int) {
        val pc = _state.value.pc ?: return
        if (pc.pcId != pcId || (pc.host == host && pc.port == port)) return
        scope.launch {
            save(pc.copy(host = host, port = port))
            onNetworkAvailable()
        }
    }

    fun sendStatus(status: DeviceStatus) {
        lastStatus = status
        if (_state.value.status == LinkStatus.Connected) {
            socket?.send(envelopeJson(MessageType.DEVICE_STATUS, DeviceStatus.serializer(), status))
        }
    }

    private suspend fun save(pc: PairedPc?) {
        settings.upsert(
            SettingEntity(KEY, pc?.let { BridgeJson.encodeToString(PairedPc.serializer(), it) } ?: "null", clock.now()),
        )
        _state.update { it.copy(pc = pc) }
    }

    private suspend fun forget() {
        retry?.cancel()
        socket = null
        save(null)
        PhoneIdentity.delete()
        _state.value = BridgeUiState()
    }

    private fun connect(pairing: PairingLink?) {
        val pc = pairing?.pc ?: _state.value.pc ?: return
        val mine = ++generation
        if (pairing == null) _state.update { it.copy(status = LinkStatus.Connecting) }
        val request = Request.Builder().url("wss://${pc.host}:${pc.port}/").build()
        socket = runCatching { bridgeHttpClient(pc.certFp) }
            .getOrElse {
                _state.update { s -> s.copy(status = LinkStatus.Offline, problem = it.message) }
                return
            }
            .newWebSocket(request, Listener(mine, pc, pairing))
    }

    private fun scheduleRetry() {
        if (!running || _state.value.pc == null) return
        retry?.cancel()
        retry = scope.launch {
            delay(backoff.nextDelay())
            connect(null)
        }
    }

    private inner class Listener(
        private val mine: Int,
        private val pc: PairedPc,
        private val pairing: PairingLink?,
    ) : WebSocketListener() {
        private fun current() = mine == generation

        override fun onOpen(webSocket: WebSocket, response: Response) {
            val first = if (pairing != null) {
                envelopeJson(
                    MessageType.PAIR_REQUEST,
                    PairRequest.serializer(),
                    PairRequest(pairing.token, deviceName(), BuildConfig.VERSION_NAME),
                )
            } else {
                envelopeJson(MessageType.HELLO, Hello.serializer(), Hello(deviceName(), BuildConfig.VERSION_NAME))
            }
            webSocket.send(first)
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            if (!current()) return
            val envelope = parseEnvelope(text) ?: return
            if (envelope.type.startsWith("file.")) {
                runCatching { files.onMessage(envelope.type, envelope.body) }
                return
            }
            when (val reaction = react(envelope)) {
                is Reaction.Paired -> scope.launch {
                    save(pc.copy(pcId = reaction.pcId.ifBlank { pc.pcId }, pcName = reaction.pcName.ifBlank { pc.pcName }))
                    running = true
                    online(webSocket)
                }
                Reaction.Connected -> online(webSocket)
                is Reaction.Rejected -> {
                    if (reaction.forget || pairing != null) {
                        scope.launch {
                            if (reaction.forget) forget()
                            _state.update { it.copy(status = if (it.pc == null) LinkStatus.NotPaired else LinkStatus.Offline, problem = reaction.reason) }
                        }
                    }
                }
                Reaction.Unpaired -> scope.launch { forget() }
                is Reaction.ReplyPong -> webSocket.send(envelopeJson(MessageType.PONG, Pong.serializer(), Pong(reaction.replyTo)))
                Reaction.Ignore -> Unit
            }
        }

        private fun online(webSocket: WebSocket) {
            backoff.reset()
            files.outbox = Outbox { webSocket.send(it) }
            _state.update { it.copy(status = LinkStatus.Connected, problem = null) }
            lastStatus?.let { webSocket.send(envelopeJson(MessageType.DEVICE_STATUS, DeviceStatus.serializer(), it)) }
        }

        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) = dropped(null)

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) = dropped(t.message)

        private fun dropped(problem: String?) {
            if (!current()) return
            socket = null
            files.onDisconnected()
            _state.update {
                when {
                    it.pc == null -> it.copy(status = LinkStatus.NotPaired, problem = problem ?: it.problem)
                    else -> it.copy(status = LinkStatus.Offline, problem = problem ?: it.problem)
                }
            }
            scheduleRetry()
        }
    }

    private companion object {
        const val KEY = "bridge"

        fun deviceName(): String =
            "${Build.MANUFACTURER.replaceFirstChar { it.uppercase() }} ${Build.MODEL}".trim().take(60)
    }
}
