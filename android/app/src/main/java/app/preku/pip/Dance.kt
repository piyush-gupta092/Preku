package app.preku.pip

import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.sin

// Pip's dance moves (port of `desktop/src/pip/dance.ts`): pure (move, time) → frame, 120 BPM,
// a new move every 8 s, crossfading between moves so Pip never jumps.

const val BEAT_MS = 500f
const val MOVE_MS = 8_000f
const val BLEND_MS = 350f

enum class DanceMove { Robot, Gangnam, Bhangra, Floss, Moonwalk, Naatu, Dougie, RunningMan }

data class Foot(val dx: Float = 0f, val lift: Float = 0f)

data class DanceFrame(
    val x: Float,
    val y: Float,
    val tilt: Float,
    val armL: Float,
    val armR: Float,
    val footL: Foot,
    val footR: Foot,
)

private const val TAU = (PI * 2).toFloat()
private const val PI_F = PI.toFloat()
private fun bump(phase: Float) = (1f - cos(phase * TAU)) / 2f
private fun frac(x: Float) = x - floor(x)
private val STILL = Foot()

/** 0 → 1 → -1 → 0 over one period, in straight lines (constant glide speed). */
fun triangle(p: Float): Float {
    val f = frac(p)
    return when {
        f < 0.25f -> f * 4f
        f < 0.75f -> 2f - f * 4f
        else -> f * 4f - 4f
    }
}

fun currentMove(time: Float): DanceMove {
    val moves = DanceMove.entries
    val index = floor(time / MOVE_MS).toInt() % moves.size
    return moves[(index + moves.size) % moves.size]
}

private fun lerp(a: Float, b: Float, k: Float) = a + (b - a) * k

fun danceAt(time: Float): DanceFrame {
    val frame = danceFrame(currentMove(time), time)
    val since = ((time % MOVE_MS) + MOVE_MS) % MOVE_MS
    if (since >= BLEND_MS) return frame
    val previous = danceFrame(currentMove(time - BLEND_MS), time)
    val k = (1f - cos(since / BLEND_MS * PI_F)) / 2f
    return DanceFrame(
        x = lerp(previous.x, frame.x, k),
        y = lerp(previous.y, frame.y, k),
        tilt = lerp(previous.tilt, frame.tilt, k),
        armL = lerp(previous.armL, frame.armL, k),
        armR = lerp(previous.armR, frame.armR, k),
        footL = Foot(lerp(previous.footL.dx, frame.footL.dx, k), lerp(previous.footL.lift, frame.footL.lift, k)),
        footR = Foot(lerp(previous.footR.dx, frame.footR.dx, k), lerp(previous.footR.lift, frame.footR.lift, k)),
    )
}

fun danceFrame(move: DanceMove, time: Float): DanceFrame {
    val beat = time / BEAT_MS
    val phase = frac(beat)
    val half = frac(beat * 2f)
    val even = floor(beat).toInt() % 2 == 0

    return when (move) {
        DanceMove.Robot -> {
            // Snap between poses on every half beat, no easing.
            val step = floor(beat * 2f).toInt() % 4
            val poses = listOf(1.6f to 0.3f, 0.3f to 1.6f, 2.4f to 2.4f, 1.6f to 1.6f)
            val (armL, armR) = poses[(step + 4) % 4]
            DanceFrame(if (step % 2 == 0) -3f else 3f, 0f, 0f, armL, armR, STILL, STILL)
        }
        DanceMove.Gangnam -> {
            val lasso = if (floor(beat).toInt() % 4 == 3) sin(phase * PI_F) else 0f
            val gallop = bump(half) * 4f
            DanceFrame(
                0f, gallop, 0f, 0.9f, 0.9f + lasso * (1.9f + sin(time / 60f) * 0.3f),
                Foot(0f, if (even) gallop else 0f), Foot(0f, if (even) 0f else gallop),
            )
        }
        DanceMove.Bhangra -> {
            val bounce = bump(phase)
            DanceFrame(
                -2f * cos(beat * PI_F), bounce * 5f, -0.08f * cos(beat * PI_F),
                2.7f + bounce * 0.3f, 2.7f + bounce * 0.3f,
                Foot(0f, if (even) bounce * 5f else 0f), Foot(0f, if (even) 0f else bounce * 5f),
            )
        }
        DanceMove.Floss -> {
            val swing = sin(beat * PI_F)
            DanceFrame(-swing * 4f, 0f, swing * 0.06f, 0.5f + swing * 0.45f, 0.5f - swing * 0.45f, STILL, STILL)
        }
        DanceMove.Moonwalk -> {
            val slide = bump(phase)
            val heel = (1f - cos(beat * PI_F)) / 2f
            DanceFrame(
                -8f * triangle(beat / 8f), 0f, -0.05f, 0.45f, 0.65f,
                Foot(if (even) -slide * 3f else 0f, 3f * heel),
                Foot(if (even) 0f else slide * 3f, 3f * (1f - heel)),
            )
        }
        DanceMove.Naatu -> {
            val quick = frac(beat * 4f)
            val left = floor(beat * 4f).toInt() % 2 == 0
            val lift = bump(quick) * 4f
            DanceFrame(
                0f, bump(half) * 2f, 0f, 0.5f, 0.5f,
                if (left) Foot(lift * 0.375f, lift) else STILL,
                if (left) STILL else Foot(lift * 0.375f, lift),
            )
        }
        DanceMove.Dougie -> {
            val sway = sin(beat * PI_F)
            DanceFrame(sway * 3f, bump(phase) * 1.5f, sway * 0.15f, 0.6f, 2.5f + sin(beat * TAU) * 0.5f, STILL, STILL)
        }
        DanceMove.RunningMan -> {
            val lift = bump(phase) * 7f
            val slide = bump(phase) * 4f
            val pump = cos(beat * PI_F)
            DanceFrame(
                0f, bump(phase) * 2f, 0f, 0.75f + pump * 0.45f, 0.75f - pump * 0.45f,
                if (even) Foot(0f, lift) else Foot(-slide, 0f),
                if (even) Foot(slide, 0f) else Foot(0f, lift),
            )
        }
    }
}
