package app.preku.bridge

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.util.Base64
import app.preku.bridge.protocol.FileAccept
import app.preku.bridge.protocol.FileAck
import app.preku.bridge.protocol.FileCancel
import app.preku.bridge.protocol.FileChunk
import app.preku.bridge.protocol.FileDecline
import app.preku.bridge.protocol.FileDone
import app.preku.bridge.protocol.FileOffer
import app.preku.bridge.protocol.FileResult
import app.preku.bridge.protocol.MessageType
import app.preku.di.ApplicationScope
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import java.io.InputStream
import java.io.OutputStream
import java.security.MessageDigest
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.JsonObject

// File sharing on the phone: same protocol as the PC (`desktop/src-tauri/src/modules/bridge/
// files.rs`): offer → accept → chunks (192 KiB, ≤ 4 unacknowledged) → done → result.
// Received files go to Download/Preku.

const val CHUNK = 192 * 1024
const val WINDOW = 4
const val MAX_SIZE = 8L * 1024 * 1024 * 1024

enum class Direction { Send, Receive }
enum class TransferState { Waiting, Active, Done, Failed }

data class TransferUi(
    val id: String,
    val name: String,
    val size: Long,
    val bytes: Long,
    val direction: Direction,
    val state: TransferState,
    val error: String? = null,
    /** Where a received file was saved (to open it). */
    val uri: Uri? = null,
)

/** Same rules as the desktop's `sanitize_name`. */
fun sanitizeName(name: String): String {
    val base = name.substringAfterLast('/').substringAfterLast('\\')
    val cleaned = base.map { if (it.isISOControl() || it in "<>:\"/\\|?*") '_' else it }.joinToString("")
    var out = cleaned.trim().trimEnd('.', ' ').trimStart('.').take(120)
    if (out.isEmpty()) out = "file"
    val stem = out.substringBefore('.').uppercase()
    val reserved = stem in setOf("CON", "PRN", "AUX", "NUL") ||
        ((stem.startsWith("COM") || stem.startsWith("LPT")) && stem.length == 4 && stem[3].isDigit())
    return if (reserved) "_$out" else out
}

fun chunkCount(size: Long): Int = maxOf(1L, (size.coerceAtLeast(0) + CHUNK - 1) / CHUNK).toInt()

/** The connection's way of sending text frames (set while connected). */
fun interface Outbox {
    fun send(text: String): Boolean
}

@Singleton
class FileTransfers @Inject constructor(
    @ApplicationContext private val context: Context,
    @ApplicationScope private val scope: CoroutineScope,
) {
    private val _transfers = MutableStateFlow<List<TransferUi>>(emptyList())
    val transfers: StateFlow<List<TransferUi>> = _transfers.asStateFlow()

    @Volatile var outbox: Outbox? = null

    private class Incoming(
        val name: String,
        val size: Long,
        val sha256: String,
        val target: Target,
        val out: OutputStream,
        val digest: MessageDigest = MessageDigest.getInstance("SHA-256"),
        var next: Int = 0,
        var written: Long = 0,
    )

    private class Outgoing(val uri: Uri, val size: Long) {
        @Volatile var acked = 0
        @Volatile var cancelled = false
        val wake = Channel<Unit>(Channel.CONFLATED)
    }

    private val incoming = ConcurrentHashMap<String, Incoming>()
    private val outgoing = ConcurrentHashMap<String, Outgoing>()

    private fun <T> send(type: String, serializer: KSerializer<T>, body: T) {
        outbox?.send(envelopeJson(type, serializer, body))
    }

    private fun upsert(item: TransferUi) = _transfers.update { list ->
        (listOf(item) + list.filterNot { it.id == item.id }).take(HISTORY)
    }

    private fun change(id: String, f: (TransferUi) -> TransferUi) = _transfers.update { list ->
        list.map { if (it.id == id) f(it) else it }
    }

    private fun fail(id: String, reason: String) = change(id) { it.copy(state = TransferState.Failed, error = reason) }

    // ---- Sending ----

    /** Sends files picked or shared on the phone. Needs a connected PC. */
    fun send(uris: List<Uri>) {
        for (uri in uris) scope.launch(Dispatchers.IO) { offer(uri, null) }
    }

    /** Sends local copies (from the share sheet), keeping their names. */
    fun sendCopies(files: List<File>) {
        for (file in files) scope.launch(Dispatchers.IO) { offer(Uri.fromFile(file), file.name) }
    }

    private fun offer(uri: Uri, knownName: String?) {
        val resolver = context.contentResolver
        var name = knownName ?: "file"
        var size = -1L
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                if (knownName == null) c.getString(0)?.let { name = it }
                if (!c.isNull(1)) size = c.getLong(1)
            }
        }
        val id = UUID.randomUUID().toString()
        upsert(TransferUi(id, name, size.coerceAtLeast(0), 0, Direction.Send, TransferState.Waiting))
        val hashed = runCatching {
            resolver.openInputStream(uri)!!.use { input -> hash(input) }
        }.getOrElse {
            fail(id, "error")
            return
        }
        val (sha, measured) = hashed
        if (measured > MAX_SIZE) {
            fail(id, "too_large")
            return
        }
        change(id) { it.copy(size = measured) }
        if (outbox == null) {
            fail(id, "disconnected")
            return
        }
        outgoing[id] = Outgoing(uri, measured)
        send(MessageType.FILE_OFFER, FileOffer.serializer(), FileOffer(id, name, measured, resolver.getType(uri), sha))
    }

    private fun hash(input: InputStream): Pair<String, Long> {
        val digest = MessageDigest.getInstance("SHA-256")
        val buf = ByteArray(64 * 1024)
        var total = 0L
        while (true) {
            val n = input.read(buf)
            if (n < 0) break
            digest.update(buf, 0, n)
            total += n
        }
        return digest.digest().joinToString("") { String.format(java.util.Locale.ROOT, "%02x", it) } to total
    }

    private fun pump(id: String) {
        val out = outgoing[id] ?: return
        change(id) { it.copy(state = TransferState.Active) }
        scope.launch(Dispatchers.IO) {
            val ok = runCatching {
                context.contentResolver.openInputStream(out.uri)!!.use { input ->
                    val buf = ByteArray(CHUNK)
                    for (index in 0 until chunkCount(out.size)) {
                        while (index >= out.acked + WINDOW) {
                            if (out.cancelled) return@launch
                            withTimeoutOrNull(30_000) { out.wake.receive() } ?: error("no ack")
                        }
                        if (out.cancelled) return@launch
                        var n = 0
                        while (n < CHUNK) {
                            val r = input.read(buf, n, CHUNK - n)
                            if (r < 0) break
                            n += r
                        }
                        val data = Base64.encodeToString(buf, 0, n, Base64.NO_WRAP)
                        send(MessageType.FILE_CHUNK, FileChunk.serializer(), FileChunk(id, index, data))
                    }
                    send(MessageType.FILE_DONE, FileDone.serializer(), FileDone(id))
                }
            }.isSuccess
            if (!ok) {
                outgoing.remove(id)
                send(MessageType.FILE_CANCEL, FileCancel.serializer(), FileCancel(id))
                fail(id, "error")
            }
        }
    }

    fun cancel(id: String) {
        outgoing.remove(id)?.let {
            it.cancelled = true
            it.wake.trySend(Unit)
        }
        incoming.remove(id)?.let { discard(it) }
        send(MessageType.FILE_CANCEL, FileCancel.serializer(), FileCancel(id))
        fail(id, "cancelled")
    }

    // ---- Receiving ----

    /** Where a received file is written while it arrives. */
    private sealed interface Target {
        data class Media(val uri: Uri) : Target
        data class Plain(val file: File) : Target
    }

    private fun open(name: String, mime: String?): Pair<Target, OutputStream>? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val values = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, name)
                put(MediaStore.Downloads.MIME_TYPE, mime ?: "application/octet-stream")
                put(MediaStore.Downloads.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/Preku")
                put(MediaStore.Downloads.IS_PENDING, 1)
            }
            val uri = context.contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values) ?: return null
            val stream = context.contentResolver.openOutputStream(uri) ?: return null
            Target.Media(uri) to stream
        } else {
            // Android 9: the app's own Downloads folder (no storage permission needed).
            val dir = File(context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS), "Preku").apply { mkdirs() }
            var file = File(dir, name)
            var n = 1
            while (file.exists()) {
                val dot = name.lastIndexOf('.')
                file = File(dir, if (dot > 0) "${name.substring(0, dot)} ($n)${name.substring(dot)}" else "$name ($n)")
                n++
            }
            Target.Plain(file) to file.outputStream()
        }
    }

    private fun discard(inc: Incoming) {
        runCatching { inc.out.close() }
        when (val t = inc.target) {
            is Target.Media -> runCatching { context.contentResolver.delete(t.uri, null, null) }
            is Target.Plain -> t.file.delete()
        }
    }

    /** Handles one `file.*` message from the PC (called on the socket's reader thread). */
    fun onMessage(type: String, body: JsonObject) {
        fun <T> parse(s: KSerializer<T>): T = BridgeJson.decodeFromJsonElement(s, body)
        when (type) {
            MessageType.FILE_OFFER -> onOffer(parse(FileOffer.serializer()))
            MessageType.FILE_CHUNK -> onChunk(parse(FileChunk.serializer()))
            MessageType.FILE_DONE -> onDone(parse(FileDone.serializer()).transferId)
            MessageType.FILE_ACCEPT -> pump(parse(FileAccept.serializer()).transferId)
            MessageType.FILE_ACK -> {
                val ack = parse(FileAck.serializer())
                outgoing[ack.transferId]?.let { out ->
                    out.acked = maxOf(out.acked, ack.index + 1)
                    out.wake.trySend(Unit)
                    change(ack.transferId) { it.copy(bytes = minOf(out.size, (ack.index + 1L) * CHUNK)) }
                }
            }
            MessageType.FILE_RESULT -> {
                val result = parse(FileResult.serializer())
                outgoing.remove(result.transferId)
                if (result.ok) {
                    change(result.transferId) { it.copy(state = TransferState.Done, bytes = it.size) }
                } else {
                    fail(result.transferId, result.reason ?: "error")
                }
            }
            MessageType.FILE_DECLINE -> {
                val decline = parse(FileDecline.serializer())
                outgoing.remove(decline.transferId)?.cancelled = true
                fail(decline.transferId, decline.reason)
            }
            MessageType.FILE_CANCEL -> {
                val id = parse(FileCancel.serializer()).transferId
                outgoing.remove(id)?.let {
                    it.cancelled = true
                    it.wake.trySend(Unit)
                }
                incoming.remove(id)?.let { discard(it) }
                fail(id, "cancelled")
            }
        }
    }

    private fun onOffer(offer: FileOffer) {
        if (offer.size !in 0..MAX_SIZE) {
            send(MessageType.FILE_DECLINE, FileDecline.serializer(), FileDecline(offer.transferId, "too_large"))
            return
        }
        val name = sanitizeName(offer.name)
        val opened = runCatching { open(name, offer.mime) }.getOrNull()
        if (opened == null) {
            send(MessageType.FILE_DECLINE, FileDecline.serializer(), FileDecline(offer.transferId, "error"))
            return
        }
        incoming[offer.transferId] = Incoming(name, offer.size, offer.sha256.lowercase(), opened.first, opened.second)
        upsert(TransferUi(offer.transferId, name, offer.size, 0, Direction.Receive, TransferState.Active))
        send(MessageType.FILE_ACCEPT, FileAccept.serializer(), FileAccept(offer.transferId))
    }

    private fun onChunk(chunk: FileChunk) {
        val inc = incoming[chunk.transferId] ?: return
        val data = runCatching { Base64.decode(chunk.data, Base64.DEFAULT) }.getOrNull()
        val problem = when {
            data == null -> "error"
            chunk.index != inc.next -> "order"
            inc.written + data.size > inc.size -> "size"
            else -> null
        }
        val written = problem == null && runCatching { inc.out.write(data!!) }.isSuccess
        if (!written) {
            incoming.remove(chunk.transferId)
            discard(inc)
            val reason = problem ?: "write"
            send(MessageType.FILE_RESULT, FileResult.serializer(), FileResult(chunk.transferId, false, reason))
            fail(chunk.transferId, reason)
            return
        }
        inc.digest.update(data!!)
        inc.written += data.size
        inc.next++
        send(MessageType.FILE_ACK, FileAck.serializer(), FileAck(chunk.transferId, chunk.index))
        change(chunk.transferId) { it.copy(bytes = inc.written) }
    }

    private fun onDone(id: String) {
        val inc = incoming.remove(id) ?: return
        val closed = runCatching { inc.out.close() }.isSuccess
        val hash = inc.digest.digest().joinToString("") { String.format(java.util.Locale.ROOT, "%02x", it) }
        val reason = when {
            !closed -> "write"
            inc.written != inc.size -> "size"
            hash != inc.sha256 -> "checksum"
            else -> null
        }
        if (reason != null) {
            discard(inc)
            send(MessageType.FILE_RESULT, FileResult.serializer(), FileResult(id, false, reason))
            fail(id, reason)
            return
        }
        val uri = when (val t = inc.target) {
            is Target.Media -> {
                context.contentResolver.update(
                    t.uri,
                    ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) },
                    null,
                    null,
                )
                t.uri
            }
            is Target.Plain -> null
        }
        send(MessageType.FILE_RESULT, FileResult.serializer(), FileResult(id, true))
        change(id) { it.copy(state = TransferState.Done, bytes = it.size, uri = uri) }
    }

    /** The connection dropped: everything in flight fails, partial files are removed. */
    fun onDisconnected() {
        outbox = null
        for ((id, out) in outgoing) {
            out.cancelled = true
            out.wake.trySend(Unit)
            fail(id, "disconnected")
        }
        outgoing.clear()
        for ((id, inc) in incoming) {
            discard(inc)
            fail(id, "disconnected")
        }
        incoming.clear()
    }

    private companion object {
        const val HISTORY = 12
    }
}
