package app.preku.di

import android.content.Context
import androidx.room.Room
import app.preku.data.FocusNotifier
import app.preku.data.FocusSessionDao
import app.preku.data.HabitDao
import app.preku.data.PrekuDatabase
import app.preku.data.RoomSettingsRepository
import app.preku.data.RoomTaskRepository
import app.preku.data.RoomTimerStorage
import app.preku.data.RoomWellbeingRepository
import app.preku.data.SettingDao
import app.preku.data.SettingsRepository
import app.preku.data.TaskDao
import app.preku.data.TaskRepository
import app.preku.data.TimerStorage
import app.preku.data.WaterDao
import app.preku.data.WellbeingRepository
import app.preku.domain.Clock
import app.preku.domain.SystemClock
import app.preku.island.AndroidIslandController
import app.preku.ui.IslandController
import app.preku.platform.AndroidFocusNotifier
import app.preku.platform.AndroidPermissions
import app.preku.platform.PermissionChecker
import app.preku.platform.ScreenTimeSource
import app.preku.platform.UsageStatsScreenTime
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Qualifier
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/** A scope that lives as long as the app (timer ticks, settings writes). */
@Qualifier
@Retention(AnnotationRetention.BINARY)
annotation class ApplicationScope

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {
    @Provides
    @Singleton
    fun provideDatabase(@ApplicationContext context: Context): PrekuDatabase =
        Room.databaseBuilder(context, PrekuDatabase::class.java, PrekuDatabase.NAME).build()

    @Provides fun provideSettingDao(db: PrekuDatabase): SettingDao = db.settingDao()
    @Provides fun provideTaskDao(db: PrekuDatabase): TaskDao = db.taskDao()
    @Provides fun provideFocusSessionDao(db: PrekuDatabase): FocusSessionDao = db.focusSessionDao()
    @Provides fun provideWaterDao(db: PrekuDatabase): WaterDao = db.waterDao()
    @Provides fun provideHabitDao(db: PrekuDatabase): HabitDao = db.habitDao()

    @Provides fun provideClock(): Clock = SystemClock

    @Provides
    @Singleton
    @ApplicationScope
    fun provideApplicationScope(): CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
}

@Module
@InstallIn(SingletonComponent::class)
abstract class BindingsModule {
    @Binds abstract fun tasks(impl: RoomTaskRepository): TaskRepository
    @Binds abstract fun wellbeing(impl: RoomWellbeingRepository): WellbeingRepository
    @Binds @Singleton abstract fun settings(impl: RoomSettingsRepository): SettingsRepository
    @Binds abstract fun timerStorage(impl: RoomTimerStorage): TimerStorage
    @Binds abstract fun focusNotifier(impl: AndroidFocusNotifier): FocusNotifier
    @Binds abstract fun screenTime(impl: UsageStatsScreenTime): ScreenTimeSource
    @Binds abstract fun permissions(impl: AndroidPermissions): PermissionChecker
    @Binds abstract fun island(impl: AndroidIslandController): IslandController
}
