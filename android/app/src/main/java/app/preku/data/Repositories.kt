package app.preku.data

import app.preku.domain.Clock
import app.preku.domain.STREAK_MILESTONES
import app.preku.domain.dayKey
import app.preku.domain.streak
import java.time.LocalDate
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map

data class Task(val id: String, val title: String, val done: Boolean)

interface TaskRepository {
    val tasks: Flow<List<Task>>
    suspend fun add(title: String)
    suspend fun setDone(id: String, done: Boolean)
    suspend fun delete(id: String)
}

@Singleton
class RoomTaskRepository @Inject constructor(
    private val dao: TaskDao,
    private val clock: Clock,
) : TaskRepository {
    override val tasks: Flow<List<Task>> =
        dao.observe().map { rows -> rows.map { Task(it.id, it.title, it.done) } }

    override suspend fun add(title: String) {
        val trimmed = title.trim().take(MAX_TITLE)
        require(trimmed.isNotEmpty()) { "A task needs a title" }
        val now = clock.now()
        dao.insert(TaskEntity(UUID.randomUUID().toString(), trimmed, false, dao.nextOrder(), now, now))
    }

    override suspend fun setDone(id: String, done: Boolean) = dao.setDone(id, done, clock.now())

    override suspend fun delete(id: String) = dao.delete(id, clock.now())

    private companion object {
        const val MAX_TITLE = 200
    }
}

data class Habit(
    val id: String,
    val name: String,
    val target: Int,
    val countToday: Int,
    val doneToday: Boolean,
    val streak: Int,
)

interface WellbeingRepository {
    fun waterToday(): Flow<Int>
    suspend fun addWater(ml: Int)
    suspend fun undoWater()
    val habits: Flow<List<Habit>>
    suspend fun addHabit(name: String, target: Int)
    suspend fun deleteHabit(id: String)

    /** Changes today's count by [delta]; returns true when it just completed a milestone streak. */
    suspend fun logHabit(id: String, delta: Int): Boolean
}

@Singleton
class RoomWellbeingRepository @Inject constructor(
    private val water: WaterDao,
    private val habitDao: HabitDao,
    private val clock: Clock,
) : WellbeingRepository {

    override fun waterToday(): Flow<Int> = water.observeTotal(clock.today().dayKey())

    override suspend fun addWater(ml: Int) {
        require(ml in 1..2000) { "A drink is 1-2000 ml" }
        val now = clock.now()
        water.insert(WaterLogEntity(UUID.randomUUID().toString(), ml, now, clock.today().dayKey(), now))
    }

    override suspend fun undoWater() = water.undoLast(clock.today().dayKey(), clock.now())

    override val habits: Flow<List<Habit>> =
        combine(habitDao.observeHabits(), habitDao.observeLogs()) { habits, logs ->
            buildHabits(habits, logs, clock.today())
        }

    override suspend fun addHabit(name: String, target: Int) {
        val trimmed = name.trim().take(80)
        require(trimmed.isNotEmpty()) { "A habit needs a name" }
        require(target in 1..20) { "Target is 1-20 times a day" }
        val now = clock.now()
        habitDao.insert(HabitEntity(UUID.randomUUID().toString(), trimmed, target, habitDao.nextOrder(), now, now))
    }

    override suspend fun deleteHabit(id: String) = habitDao.delete(id, clock.now())

    override suspend fun logHabit(id: String, delta: Int): Boolean {
        val today = clock.today().dayKey()
        val before = habitDao.count(id, today) ?: 0
        val after = (before + delta).coerceIn(0, 99)
        habitDao.upsertLog(HabitLogEntity(id, today, after, clock.now()))
        val target = habitDao.target(id) ?: return false
        if (before >= target || after < target) return false
        val doneDays = habitDao.logsFor(id).filter { it.count >= target }.map { LocalDate.parse(it.day) }.toSet()
        return isMilestone(streak(doneDays, clock.today()), doneToday = true)
    }
}

/** Pure: habit rows + logs → what the UI shows (counts, done, streak). */
fun buildHabits(habits: List<HabitEntity>, logs: List<HabitLogEntity>, today: LocalDate): List<Habit> {
    val byHabit = logs.groupBy { it.habitId }
    val todayKey = today.dayKey()
    return habits.map { h ->
        val mine = byHabit[h.id].orEmpty()
        val doneDays = mine.filter { it.count >= h.target }.map { LocalDate.parse(it.day) }.toSet()
        val countToday = mine.firstOrNull { it.day == todayKey }?.count ?: 0
        Habit(h.id, h.name, h.target, countToday, countToday >= h.target, streak(doneDays, today))
    }
}

/** True when checking in today made the streak hit a milestone Pip celebrates. */
fun isMilestone(streak: Int, doneToday: Boolean): Boolean = doneToday && streak in STREAK_MILESTONES
