package app.preku.data

import androidx.room.ColumnInfo
import androidx.room.Dao
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Index
import androidx.room.Insert
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.RoomDatabase
import androidx.room.Upsert
import kotlinx.coroutines.flow.Flow

// Tables mirror the desktop SQLite schema (desktop/src-tauri/migrations) column for column, so
// the phone sync (Phase 8) can merge rows by id + updated_at. Deletes are tombstones
// (`deleted = 1`), never real deletes, for the same reason. Days are local "YYYY-MM-DD".

/** Key/value user settings; values are JSON documents (mirrors the desktop `settings` table). */
@Entity(tableName = "settings")
data class SettingEntity(
    @PrimaryKey val key: String,
    val valueJson: String,
    /** Unix epoch milliseconds. */
    val updatedAt: Long,
)

@Dao
interface SettingDao {
    @Query("SELECT valueJson FROM settings WHERE `key` = :key")
    suspend fun get(key: String): String?

    @Upsert
    suspend fun upsert(setting: SettingEntity)
}

@Entity(tableName = "tasks", indices = [Index("deleted", "done", "sort_order")])
data class TaskEntity(
    @PrimaryKey val id: String,
    val title: String,
    val done: Boolean,
    @ColumnInfo(name = "sort_order") val sortOrder: Int,
    @ColumnInfo(name = "created_at") val createdAt: Long,
    @ColumnInfo(name = "updated_at") val updatedAt: Long,
    val deleted: Boolean = false,
)

@Dao
interface TaskDao {
    /** Open tasks first (in order), then done ones, newest done first. */
    @Query(
        "SELECT * FROM tasks WHERE deleted = 0 " +
            "ORDER BY done ASC, CASE WHEN done = 0 THEN sort_order ELSE -updated_at END ASC",
    )
    fun observe(): Flow<List<TaskEntity>>

    @Query("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tasks")
    suspend fun nextOrder(): Int

    @Insert
    suspend fun insert(task: TaskEntity)

    @Query("UPDATE tasks SET done = :done, updated_at = :now WHERE id = :id")
    suspend fun setDone(id: String, done: Boolean, now: Long)

    @Query("UPDATE tasks SET deleted = 1, updated_at = :now WHERE id = :id")
    suspend fun delete(id: String, now: Long)
}

@Entity(tableName = "focus_sessions")
data class FocusSessionEntity(
    @PrimaryKey val id: String,
    val label: String,
    @ColumnInfo(name = "started_at") val startedAt: Long,
    @ColumnInfo(name = "duration_s") val durationS: Int,
    @ColumnInfo(name = "focused_s") val focusedS: Int,
    val completed: Boolean,
    @ColumnInfo(name = "updated_at") val updatedAt: Long,
    val deleted: Boolean = false,
)

@Dao
interface FocusSessionDao {
    @Insert
    suspend fun insert(session: FocusSessionEntity)

    @Query("SELECT COALESCE(SUM(focused_s), 0) FROM focus_sessions WHERE deleted = 0 AND started_at >= :since")
    fun observeFocusedSince(since: Long): Flow<Long>
}

@Entity(tableName = "water_logs", indices = [Index("deleted", "day")])
data class WaterLogEntity(
    @PrimaryKey val id: String,
    val ml: Int,
    /** Unix ms. */
    val at: Long,
    val day: String,
    @ColumnInfo(name = "updated_at") val updatedAt: Long,
    val deleted: Boolean = false,
)

@Dao
interface WaterDao {
    @Query("SELECT COALESCE(SUM(ml), 0) FROM water_logs WHERE deleted = 0 AND day = :day")
    fun observeTotal(day: String): Flow<Int>

    @Insert
    suspend fun insert(log: WaterLogEntity)

    /** Undo: tombstones the latest drink of the day. */
    @Query(
        "UPDATE water_logs SET deleted = 1, updated_at = :now WHERE id = " +
            "(SELECT id FROM water_logs WHERE deleted = 0 AND day = :day ORDER BY at DESC LIMIT 1)",
    )
    suspend fun undoLast(day: String, now: Long)
}

@Entity(tableName = "habits")
data class HabitEntity(
    @PrimaryKey val id: String,
    val name: String,
    /** Times per day. */
    val target: Int,
    @ColumnInfo(name = "sort_order") val sortOrder: Int,
    @ColumnInfo(name = "created_at") val createdAt: Long,
    @ColumnInfo(name = "updated_at") val updatedAt: Long,
    val deleted: Boolean = false,
)

@Entity(tableName = "habit_logs", primaryKeys = ["habit_id", "day"])
data class HabitLogEntity(
    @ColumnInfo(name = "habit_id") val habitId: String,
    val day: String,
    val count: Int,
    @ColumnInfo(name = "updated_at") val updatedAt: Long,
)

@Dao
interface HabitDao {
    @Query("SELECT * FROM habits WHERE deleted = 0 ORDER BY sort_order")
    fun observeHabits(): Flow<List<HabitEntity>>

    /** All logs (they are tiny); streaks need the history. */
    @Query("SELECT * FROM habit_logs")
    fun observeLogs(): Flow<List<HabitLogEntity>>

    @Query("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM habits")
    suspend fun nextOrder(): Int

    @Insert
    suspend fun insert(habit: HabitEntity)

    @Query("UPDATE habits SET deleted = 1, updated_at = :now WHERE id = :id")
    suspend fun delete(id: String, now: Long)

    @Query("SELECT count FROM habit_logs WHERE habit_id = :habitId AND day = :day")
    suspend fun count(habitId: String, day: String): Int?

    @Upsert
    suspend fun upsertLog(log: HabitLogEntity)

    @Query("SELECT * FROM habit_logs WHERE habit_id = :habitId")
    suspend fun logsFor(habitId: String): List<HabitLogEntity>

    @Query("SELECT target FROM habits WHERE id = :id")
    suspend fun target(id: String): Int?
}

/**
 * Version 1 is the first schema that ships (Phase 5). The Phase 0 scaffold never reached a
 * device, so it needs no migration. From here on, every change needs a Room migration.
 */
@Database(
    entities = [
        SettingEntity::class,
        TaskEntity::class,
        FocusSessionEntity::class,
        WaterLogEntity::class,
        HabitEntity::class,
        HabitLogEntity::class,
    ],
    version = 1,
    exportSchema = true,
)
abstract class PrekuDatabase : RoomDatabase() {
    abstract fun settingDao(): SettingDao
    abstract fun taskDao(): TaskDao
    abstract fun focusSessionDao(): FocusSessionDao
    abstract fun waterDao(): WaterDao
    abstract fun habitDao(): HabitDao

    companion object {
        const val NAME = "preku.db"
    }
}
