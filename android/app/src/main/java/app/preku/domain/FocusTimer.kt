package app.preku.domain

/**
 * The focus timer as a pure state machine (a port of the desktop `timer.rs`, same rules):
 * time comes in as an argument, so it is fully unit-testable and survives being rebuilt from a
 * saved [TimerSave] after the process dies.
 */
class FocusTimer(private var state: State = State.Idle) {

    sealed interface State {
        data object Idle : State
        data class Running(val session: Session, val segmentStart: Long) : State
        data class Paused(val session: Session) : State
    }

    data class Session(
        val label: String,
        val totalMs: Long,
        val startedAt: Long,
        val focusedMs: Long,
    )

    enum class Status { Idle, Running, Paused }

    data class Snapshot(
        val status: Status,
        val label: String,
        val totalS: Int,
        val remainingS: Int,
        /** When a running timer ends (ms), for ticking UIs. */
        val endsAt: Long?,
    ) {
        /** 0 → 1 as the session progresses. */
        val progress: Float get() = if (totalS == 0) 0f else 1f - remainingS.toFloat() / totalS

        companion object {
            val IDLE = Snapshot(Status.Idle, "", 0, 0, null)
        }
    }

    data class SessionRecord(
        val label: String,
        val startedAt: Long,
        val durationS: Int,
        val focusedS: Int,
        val completed: Boolean,
    )

    val isActive: Boolean get() = state !is State.Idle

    /** Starts a new session; an unfinished one is returned as abandoned. */
    fun start(label: String, minutes: Int, now: Long): SessionRecord? {
        require(minutes in MIN_MINUTES..MAX_MINUTES) {
            "Timer length must be $MIN_MINUTES-$MAX_MINUTES minutes"
        }
        val abandoned = stop(now)
        state = State.Running(
            Session(normalizeLabel(label), minutes * 60_000L, now, 0L),
            segmentStart = now,
        )
        return abandoned
    }

    fun pause(now: Long) {
        val running = state as? State.Running ?: return
        state = State.Paused(
            running.session.copy(focusedMs = running.session.focusedMs + now - running.segmentStart),
        )
    }

    fun resume(now: Long) {
        val paused = state as? State.Paused ?: return
        state = State.Running(paused.session, segmentStart = now)
    }

    /** Stops early; returns the (not completed) session, or null when idle. */
    fun stop(now: Long): SessionRecord? {
        val record = when (val s = state) {
            State.Idle -> null
            is State.Running -> record(s.session, s.segmentStart, now, completed = false)
            is State.Paused -> record(s.session, null, now, completed = false)
        }
        state = State.Idle
        return record
    }

    /** Advances time; returns the completed session when the timer runs out. */
    fun tick(now: Long): SessionRecord? {
        val running = state as? State.Running ?: return null
        if (remainingMs(running.session, running.segmentStart, now) > 0) return null
        state = State.Idle
        return record(running.session, running.segmentStart, now, completed = true)
    }

    fun snapshot(now: Long): Snapshot = when (val s = state) {
        State.Idle -> Snapshot.IDLE
        is State.Running -> {
            val remaining = remainingMs(s.session, s.segmentStart, now)
            Snapshot(
                Status.Running,
                s.session.label,
                secsCeil(s.session.totalMs),
                secsCeil(remaining),
                endsAt = now + remaining,
            )
        }
        is State.Paused -> Snapshot(
            Status.Paused,
            s.session.label,
            secsCeil(s.session.totalMs),
            secsCeil(remainingMs(s.session, null, now)),
            endsAt = null,
        )
    }

    /** What to persist so the timer survives a restart. */
    fun save(): TimerSave? = when (val s = state) {
        State.Idle -> null
        is State.Running -> TimerSave(s.session.label, s.session.totalMs, s.session.startedAt, s.session.focusedMs, s.segmentStart)
        is State.Paused -> TimerSave(s.session.label, s.session.totalMs, s.session.startedAt, s.session.focusedMs, null)
    }

    companion object {
        const val MIN_MINUTES = 1
        const val MAX_MINUTES = 180
        private const val MAX_LABEL_CHARS = 60

        fun normalizeLabel(label: String): String =
            label.trim().ifEmpty { "Focus" }.take(MAX_LABEL_CHARS)

        fun restore(save: TimerSave?): FocusTimer {
            if (save == null) return FocusTimer()
            val session = Session(save.label, save.totalMs, save.startedAt, save.focusedMs)
            return FocusTimer(
                save.segmentStart?.let { State.Running(session, it) } ?: State.Paused(session),
            )
        }

        private fun remainingMs(session: Session, runningSince: Long?, now: Long): Long {
            val focused = session.focusedMs + (runningSince?.let { now - it } ?: 0L)
            return (session.totalMs - focused).coerceAtLeast(0L)
        }

        private fun secsCeil(ms: Long): Int = ((ms + 999) / 1000).toInt()

        private fun record(session: Session, runningSince: Long?, now: Long, completed: Boolean): SessionRecord {
            val focused = session.focusedMs + (runningSince?.let { now - it } ?: 0L)
            return SessionRecord(
                label = session.label,
                startedAt = session.startedAt,
                durationS = secsCeil(session.totalMs),
                focusedS = secsCeil(focused.coerceAtMost(session.totalMs)),
                completed = completed,
            )
        }
    }
}

/** A persisted timer (`segmentStart == null` means paused). */
@kotlinx.serialization.Serializable
data class TimerSave(
    val label: String,
    val totalMs: Long,
    val startedAt: Long,
    val focusedMs: Long,
    val segmentStart: Long?,
)
