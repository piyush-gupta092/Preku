package app.preku.domain

import app.preku.data.HabitEntity
import app.preku.data.HabitLogEntity
import app.preku.data.buildHabits
import app.preku.data.isMilestone
import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

private const val MIN = 60_000L

class FocusTimerTest {
    @Test
    fun countsDownAndCompletes() {
        val timer = FocusTimer()
        timer.start("Write docs", 25, 0)
        assertEquals(FocusTimer.Status.Running, timer.snapshot(0).status)
        assertEquals(25 * 60, timer.snapshot(0).remainingS)
        assertEquals(15 * 60, timer.snapshot(10 * MIN).remainingS)
        assertNull(timer.tick(24 * MIN))
        val done = timer.tick(25 * MIN)!!
        assertTrue(done.completed)
        assertEquals(25 * 60, done.focusedS)
        assertFalse(timer.isActive)
    }

    @Test
    fun pausedTimeDoesNotCount() {
        val timer = FocusTimer()
        timer.start("", 10, 0)
        timer.pause(2 * MIN)
        assertEquals(8 * 60, timer.snapshot(30 * MIN).remainingS)
        assertEquals("Focus", timer.snapshot(30 * MIN).label)
        timer.resume(30 * MIN)
        assertEquals(7 * 60, timer.snapshot(31 * MIN).remainingS)
    }

    @Test
    fun stoppingEarlyRecordsAnUnfinishedSession() {
        val timer = FocusTimer()
        timer.start("Read", 30, 0)
        val record = timer.stop(5 * MIN)!!
        assertFalse(record.completed)
        assertEquals(5 * 60, record.focusedS)
        assertEquals(FocusTimer.Snapshot.IDLE, timer.snapshot(6 * MIN))
    }

    @Test
    fun survivesARestart() {
        val timer = FocusTimer()
        timer.start("Deep work", 20, 0)
        timer.pause(5 * MIN)
        val restored = FocusTimer.restore(timer.save())
        assertEquals(FocusTimer.Status.Paused, restored.snapshot(60 * MIN).status)
        assertEquals(15 * 60, restored.snapshot(60 * MIN).remainingS)

        val running = FocusTimer()
        running.start("Run", 1, 0)
        // Ran out while the app was closed: completes on the first tick.
        assertTrue(FocusTimer.restore(running.save()).tick(5 * MIN)!!.completed)
    }

    @Test(expected = IllegalArgumentException::class)
    fun rejectsSillyLengths() {
        FocusTimer().start("x", 0, 0)
    }

    @Test
    fun trimsLongLabels() {
        assertEquals(60, FocusTimer.normalizeLabel("a".repeat(100)).length)
    }
}

class DaysTest {
    private val today = LocalDate.of(2026, 10, 10)
    private fun days(vararg back: Long) = back.map { today.minusDays(it) }.toSet()

    @Test
    fun streaksCountBackFromTodayOrYesterday() {
        assertEquals(0, streak(emptySet(), today))
        assertEquals(3, streak(days(0, 1, 2), today))
        // Not done yet today: yesterday's streak still counts.
        assertEquals(2, streak(days(1, 2), today))
        assertEquals(0, streak(days(2, 3), today))
        assertEquals(1, streak(days(0, 2, 3), today))
    }

    @Test
    fun formatsTimes() {
        assertEquals("12:34", formatClock(754))
        assertEquals("1:02:05", formatClock(3725))
        assertEquals("0:00", formatClock(-5))
        assertEquals("1 h 30 m", formatDuration(5400))
        assertEquals("5 m", formatDuration(300))
        assertEquals("20 s", formatDuration(20))
    }

    @Test
    fun buildsHabitViews() {
        val habits = listOf(
            HabitEntity("a", "Stretch", 1, 0, 0, 0),
            HabitEntity("b", "Push-ups", 3, 1, 0, 0),
        )
        val logs = listOf(
            HabitLogEntity("a", today.toString(), 1, 0),
            HabitLogEntity("a", today.minusDays(1).toString(), 1, 0),
            HabitLogEntity("b", today.toString(), 2, 0),
        )
        val (stretch, pushUps) = buildHabits(habits, logs, today)
        assertTrue(stretch.doneToday)
        assertEquals(2, stretch.streak)
        assertFalse(pushUps.doneToday)
        assertEquals(2, pushUps.countToday)
        assertEquals(0, pushUps.streak)
        assertTrue(isMilestone(7, doneToday = true))
        assertFalse(isMilestone(8, doneToday = true))
    }
}
