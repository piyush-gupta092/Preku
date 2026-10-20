package app.preku.domain

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.Locale

/** Calendar days are stored as local `YYYY-MM-DD` strings, like the desktop. */
fun LocalDate.dayKey(): String = toString()

fun dayOf(epochMs: Long, zone: ZoneId = ZoneId.systemDefault()): LocalDate =
    Instant.ofEpochMilli(epochMs).atZone(zone).toLocalDate()

/** Local midnight at the start of `day`, in epoch ms. */
fun startOfDay(day: LocalDate, zone: ZoneId = ZoneId.systemDefault()): Long =
    day.atStartOfDay(zone).toInstant().toEpochMilli()

/**
 * Consecutive days with a check-in ending today, or yesterday when today isn't done yet
 * (so a streak doesn't look broken in the morning). Same rule as the desktop `habits::streak`.
 */
fun streak(doneDays: Set<LocalDate>, today: LocalDate): Int {
    var day = when {
        today in doneDays -> today
        today.minusDays(1) in doneDays -> today.minusDays(1)
        else -> return 0
    }
    var count = 0
    while (day in doneDays) {
        count++
        day = day.minusDays(1)
    }
    return count
}

/** Streak lengths Pip celebrates. */
val STREAK_MILESTONES = setOf(3, 7, 14, 21, 30, 50, 100)

/** 754 → "12:34", 3725 → "1:02:05". */
fun formatClock(totalSeconds: Int): String {
    val s = totalSeconds.coerceAtLeast(0)
    val h = s / 3600
    val m = (s % 3600) / 60
    val sec = s % 60
    return if (h > 0) {
        String.format(Locale.ROOT, "%d:%02d:%02d", h, m, sec)
    } else {
        String.format(Locale.ROOT, "%d:%02d", m, sec)
    }
}

/** 5400 → "1 h 30 m", 300 → "5 m", 20 → "20 s". */
fun formatDuration(totalSeconds: Long): String {
    val s = totalSeconds.coerceAtLeast(0)
    val h = s / 3600
    val m = (s % 3600) / 60
    return when {
        h > 0 && m > 0 -> "$h h $m m"
        h > 0 -> "$h h"
        m > 0 -> "$m m"
        else -> "$s s"
    }
}
