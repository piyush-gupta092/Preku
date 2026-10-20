package app.preku.pip

import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/** One frame of Pip (port of `PipPose` in `renderer.ts`). */
data class PipPose(
    val mood: PipMood,
    val breathe: Float = 1f,
    val squish: Float = 1f,
    val hop: Float = 0f,
    val eyeOpen: Float = 1f,
    val lookX: Float = 0f,
    val lookY: Float = 0f,
    val time: Float = 0f,
    /** Reduced motion: no shaking, tilting or drifting extras. */
    val still: Boolean = false,
    val dance: DanceFrame? = null,
)

/**
 * Turns time + taps into a pose (port of `animator.ts`). Pure logic, so it is unit-tested;
 * [random] is injectable to make blinks deterministic.
 */
class PipAnimator(now: Long, private val random: () -> Double = Math::random) {
    private var nextBlinkAt = now + blinkGap()
    private var blinkStartedAt = Long.MIN_VALUE / 2
    private var squishStartedAt = Long.MIN_VALUE / 2
    private var recentTaps = emptyList<Long>()
    private var dizzyUntil = Long.MIN_VALUE / 2
    private var lookX = 0f
    private var lookY = 0f

    private fun blinkGap(): Long = (3000 + random() * 4000).toLong()

    fun poke(now: Long) {
        squishStartedAt = now
        recentTaps = recentTaps.filter { now - it < DIZZY_WINDOW_MS } + now
        if (recentTaps.size >= DIZZY_TAPS) {
            dizzyUntil = now + DIZZY_MS
            recentTaps = emptyList()
        }
    }

    fun look(x: Float, y: Float) {
        lookX = x.coerceIn(-1f, 1f)
        lookY = y.coerceIn(-1f, 1f)
    }

    fun isDizzy(now: Long): Boolean = now < dizzyUntil

    fun pose(now: Long, mood: PipMood, reducedMotion: Boolean): PipPose {
        if (now >= nextBlinkAt) {
            blinkStartedAt = now
            nextBlinkAt = now + blinkGap()
        }
        val requested = if (mood == PipMood.Dance && reducedMotion) PipMood.Happy else mood
        val effective = if (isDizzy(now)) PipMood.Dizzy else requested
        val dancing = effective == PipMood.Dance
        val t = (now % TIME_WRAP_MS).toFloat()
        return PipPose(
            mood = effective,
            breathe = if (reducedMotion) 1f else breathe(t),
            squish = if (reducedMotion) 1f else squish(now - squishStartedAt),
            hop = if (reducedMotion || dancing) 0f else expressionOf(effective).bob?.invoke(t) ?: 0f,
            eyeOpen = blink(now - blinkStartedAt),
            lookX = lookX,
            lookY = lookY,
            time = t,
            still = reducedMotion,
            dance = if (dancing) danceAt(t) else null,
        )
    }

    companion object {
        private const val BLINK_MS = 140L
        private const val SQUISH_MS = 280L
        private const val BREATHE_PERIOD_MS = 3200f
        private const val DIZZY_TAPS = 5
        private const val DIZZY_WINDOW_MS = 2000L
        private const val DIZZY_MS = 1500L
        /**
         * Time wraps every 16 min: a whole number of dance cycles (8 moves × 8 s) and breaths
         * (8 s), and small enough that Float time stays precise for fast motions.
         */
        private const val TIME_WRAP_MS = 960_000L

        fun breathe(t: Float): Float = 1f + 0.015f * (1f - cos(2f * PI.toFloat() * t / BREATHE_PERIOD_MS))

        /** Eyelid closes and reopens over BLINK_MS. */
        fun blink(sinceStart: Long): Float {
            if (sinceStart < 0 || sinceStart >= BLINK_MS) return 1f
            return 1f - sin(PI.toFloat() * sinceStart / BLINK_MS)
        }

        /** 1 → 0.85 → 1.05 → 1 over SQUISH_MS. */
        fun squish(sinceStart: Long): Float {
            if (sinceStart < 0 || sinceStart >= SQUISH_MS) return 1f
            val p = sinceStart.toFloat() / SQUISH_MS
            return when {
                p < 0.35f -> 1f - 0.15f * (p / 0.35f)
                p < 0.7f -> 0.85f + 0.2f * ((p - 0.35f) / 0.35f)
                else -> 1.05f - 0.05f * ((p - 0.7f) / 0.3f)
            }
        }
    }
}
