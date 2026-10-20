package app.preku.data

import app.preku.di.ApplicationScope
import app.preku.domain.Clock
import app.preku.domain.FocusTimer
import app.preku.domain.TimerSave
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.json.Json

/** Where the timer keeps its state and finished sessions. */
interface TimerStorage {
    suspend fun load(): TimerSave?
    suspend fun save(save: TimerSave?)
    suspend fun record(session: FocusTimer.SessionRecord)
}

/** Tells the user a session ended (a notification on Android). */
fun interface FocusNotifier {
    fun focusDone(label: String)
}

/**
 * One timer for the whole app (screens, the floating island and the notification all read it).
 * The state is saved on every change, so the timer survives the app being closed.
 */
@Singleton
class FocusTimerController @Inject constructor(
    private val storage: TimerStorage,
    private val notifier: FocusNotifier,
    private val clock: Clock,
    @ApplicationScope private val scope: CoroutineScope,
) {
    private val mutex = Mutex()
    private var timer = FocusTimer()
    private var ticker: Job? = null
    private val _state = MutableStateFlow(FocusTimer.Snapshot.IDLE)
    val state: StateFlow<FocusTimer.Snapshot> = _state.asStateFlow()
    private val _completed = MutableSharedFlow<FocusTimer.SessionRecord>(extraBufferCapacity = 1)

    /** Fires when a session runs to the end (Pip celebrates). */
    val completed: SharedFlow<FocusTimer.SessionRecord> = _completed.asSharedFlow()

    init {
        scope.launch {
            mutex.withLock { timer = FocusTimer.restore(storage.load()) }
            // A timer that ran out while the app was closed completes now.
            change { }
        }
    }

    fun start(label: String, minutes: Int) = scope.launch {
        change { now -> timer.start(label, minutes, now)?.let { storage.record(it) } }
    }

    fun pause() = scope.launch { change { now -> timer.pause(now) } }

    fun resume() = scope.launch { change { now -> timer.resume(now) } }

    fun stop() = scope.launch { change { now -> timer.stop(now)?.let { storage.record(it) } } }

    private suspend fun change(action: suspend (now: Long) -> Unit) {
        mutex.withLock {
            val now = clock.now()
            action(now)
            timer.tick(now)?.let { finish(it) }
            storage.save(timer.save())
            _state.value = timer.snapshot(now)
        }
        ensureTicking()
    }

    private suspend fun finish(record: FocusTimer.SessionRecord) {
        storage.record(record)
        notifier.focusDone(record.label)
        _completed.tryEmit(record)
    }

    /** Updates the countdown every second while a session runs. */
    private fun ensureTicking() {
        if (_state.value.status != FocusTimer.Status.Running) {
            ticker?.cancel()
            ticker = null
            return
        }
        if (ticker?.isActive == true) return
        ticker = scope.launch {
            while (true) {
                delay(TICK_MS)
                val stillRunning = mutex.withLock {
                    val now = clock.now()
                    val done = timer.tick(now)
                    if (done != null) {
                        finish(done)
                        storage.save(null)
                    }
                    _state.value = timer.snapshot(now)
                    _state.value.status == FocusTimer.Status.Running
                }
                if (!stillRunning) break
            }
        }
    }

    private companion object {
        const val TICK_MS = 1_000L
    }
}

class RoomTimerStorage @Inject constructor(
    private val settings: SettingDao,
    private val sessions: FocusSessionDao,
    private val clock: Clock,
) : TimerStorage {
    override suspend fun load(): TimerSave? =
        settings.get(KEY)?.let { runCatching { JSON.decodeFromString(SAVE, it) }.getOrNull() }

    override suspend fun save(save: TimerSave?) =
        settings.upsert(SettingEntity(KEY, JSON.encodeToString(SAVE, save), clock.now()))

    override suspend fun record(session: FocusTimer.SessionRecord) = sessions.insert(
        FocusSessionEntity(
            id = UUID.randomUUID().toString(),
            label = session.label,
            startedAt = session.startedAt,
            durationS = session.durationS,
            focusedS = session.focusedS,
            completed = session.completed,
            updatedAt = clock.now(),
        ),
    )

    private companion object {
        const val KEY = "timer"
        val JSON = Json { ignoreUnknownKeys = true }
        val SAVE = TimerSave.serializer().nullable
    }
}
