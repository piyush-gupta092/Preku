package app.preku.domain

import java.time.LocalDate
import java.time.LocalTime

/** Time source, injected so ViewModels and the timer are testable. */
interface Clock {
    fun now(): Long
    fun today(): LocalDate
    fun timeOfDay(): LocalTime
}

object SystemClock : Clock {
    override fun now(): Long = System.currentTimeMillis()
    override fun today(): LocalDate = LocalDate.now()
    override fun timeOfDay(): LocalTime = LocalTime.now()
}
