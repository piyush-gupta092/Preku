package app.preku.pip

import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.hypot
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The same rules the desktop tests check, so Pip behaves alike on both platforms. */
class PipLogicTest {
    @Test
    fun everyMoodHasARecipeAndArmsStayInRange() {
        assertEquals(PipMood.entries.toSet(), EXPRESSIONS.keys)
        for ((mood, spec) in EXPRESSIONS) {
            var t = 0f
            while (t < 4000f) {
                for (side in listOf(-1, 1)) {
                    val a = spec.arms(t, side)
                    assertTrue("$mood arm $a", a in 0f..PI.toFloat())
                }
                val hop = spec.bob?.invoke(t) ?: 0f
                assertTrue("$mood hop", hop in 0f..1f)
                t += 137f
            }
        }
        assertEquals(20, GALLERY.size)
    }

    @Test
    fun ikKeepsLimbLengthsAndBendsOutwards() {
        val start = Pt(10f, -60f)
        for (side in listOf(-1, 1)) {
            val limb = solveLimb(start, Pt(side * 14f, -10f), 30f to 29f, side)
            assertEquals(30f, hypot(limb.mid.x - start.x, limb.mid.y - start.y), 0.01f)
            assertEquals(29f, hypot(limb.end.x - limb.mid.x, limb.end.y - limb.mid.y), 0.01f)
            assertTrue(limb.mid.x * side > (start.x + limb.end.x) / 2 * side)
        }
    }

    @Test
    fun raisedArmsGoUpAndOut() {
        val up = armFromAngle(Pt(17f, -100f), 2.8f, 27f to 25f, 1)
        assertTrue(up.end.y < -100f)
        assertTrue(up.mid.x > 17f)
        val down = armFromAngle(Pt(17f, -100f), 0.3f, 27f to 25f, 1)
        assertTrue(down.end.y > -100f)
    }

    @Test
    fun movesNeverJumpExceptTheRobot() {
        for (move in DanceMove.entries.filter { it != DanceMove.Robot }) {
            var prev = danceFrame(move, 0f)
            var t = 16f
            while (t < MOVE_MS) {
                val f = danceFrame(move, t)
                assertTrue("$move x at $t", abs(f.x - prev.x) < 0.6f)
                assertTrue("$move armR at $t", abs(f.armR - prev.armR) < 0.35f)
                prev = f
                t += 16f
            }
        }
    }

    @Test
    fun moonwalkGlidesThereAndBack() {
        var wentLeft = false
        var cameBack = false
        var prev = danceFrame(DanceMove.Moonwalk, 0f).x
        var t = 16f
        while (t < MOVE_MS) {
            val x = danceFrame(DanceMove.Moonwalk, t).x
            if (x < prev) wentLeft = true
            if (wentLeft && x > prev) cameBack = true
            prev = x
            t += 16f
        }
        assertTrue(wentLeft && cameBack)
    }

    @Test
    fun movesBlendIntoEachOther() {
        val a = danceAt(MOVE_MS * 3 - 8f)
        val b = danceAt(MOVE_MS * 3 + 8f)
        assertTrue(abs(a.x - b.x) < 1f)
    }

    @Test
    fun fiveQuickTapsMakePipDizzy() {
        val animator = PipAnimator(0) { 0.5 }
        repeat(5) { animator.poke(it * 100L) }
        assertEquals(PipMood.Dizzy, animator.pose(600, PipMood.Idle, reducedMotion = false).mood)
        assertEquals(PipMood.Idle, animator.pose(5_000, PipMood.Idle, reducedMotion = false).mood)
    }

    @Test
    fun reducedMotionMeansNoDancing() {
        val pose = PipAnimator(0) { 0.5 }.pose(1_000, PipMood.Dance, reducedMotion = true)
        assertEquals(PipMood.Happy, pose.mood)
        assertTrue(pose.still)
        assertEquals(null, pose.dance)
    }

    @Test
    fun parsesStylesLeniently() {
        assertEquals(MascotStyle.Ghost, MascotStyle.parse("ghost"))
        assertEquals(MascotStyle.Classic, MascotStyle.parse("nonsense"))
    }
}
