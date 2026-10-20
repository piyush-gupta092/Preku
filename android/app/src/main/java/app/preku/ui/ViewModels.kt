package app.preku.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.preku.data.AppSettings
import app.preku.data.FocusTimerController
import app.preku.data.Habit
import app.preku.data.IslandSpot
import app.preku.data.SettingsRepository
import app.preku.data.Task
import app.preku.data.TaskRepository
import app.preku.data.WellbeingRepository
import app.preku.domain.Clock
import app.preku.domain.FocusTimer
import app.preku.pip.MascotStyle
import app.preku.pip.PipMood
import app.preku.platform.PermissionChecker
import app.preku.platform.PermissionState
import app.preku.platform.ScreenTimeSource
import app.preku.platform.ScreenTimeToday
import dagger.hilt.android.lifecycle.HiltViewModel
import java.time.LocalTime
import javax.inject.Inject
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private val WHILE_SUBSCRIBED = SharingStarted.WhileSubscribed(5_000)

/** How Pip looks on the home screen. Pure, so it is unit-tested. */
fun homeMood(
    timer: FocusTimer.Status,
    celebrating: Boolean,
    waterMl: Int,
    waterGoalMl: Int,
    time: LocalTime,
): PipMood = when {
    celebrating -> PipMood.Celebrate
    timer == FocusTimer.Status.Running -> PipMood.Focus
    time.hour >= 23 || time.hour < 6 -> PipMood.Sleepy
    waterMl >= waterGoalMl -> PipMood.Happy
    else -> PipMood.Idle
}

data class MascotLook(val style: MascotStyle, val color: String)

fun AppSettings.look() = MascotLook(MascotStyle.parse(mascotStyle), mascotColor)

data class HomeUiState(
    val timer: FocusTimer.Snapshot = FocusTimer.Snapshot.IDLE,
    val nextTask: String? = null,
    val openTasks: Int = 0,
    val waterMl: Int = 0,
    val waterGoalMl: Int = 2000,
    val habitsDone: Int = 0,
    val habitsTotal: Int = 0,
    /** Null when usage access isn't granted (the card then offers to set it up). */
    val screenTimeS: Long? = null,
    val mood: PipMood = PipMood.Idle,
    val look: MascotLook = AppSettings().look(),
)

@HiltViewModel
class HomeViewModel @Inject constructor(
    timer: FocusTimerController,
    tasks: TaskRepository,
    wellbeing: WellbeingRepository,
    settings: SettingsRepository,
    private val screenTime: ScreenTimeSource,
    private val clock: Clock,
) : ViewModel() {
    private val celebrating = MutableStateFlow(false)
    private val screenTimeS = MutableStateFlow<Long?>(null)

    init {
        viewModelScope.launch {
            timer.completed.collect {
                celebrating.value = true
                delay(CELEBRATE_MS)
                celebrating.value = false
            }
        }
    }

    private val summary = combine(tasks.tasks, wellbeing.waterToday(), wellbeing.habits) { t, water, habits ->
        Triple(t, water, habits)
    }

    val state: StateFlow<HomeUiState> = combine(
        timer.state,
        summary,
        settings.settings,
        celebrating,
        screenTimeS,
    ) { snapshot, (taskList, water, habits), prefs, party, screen ->
        val open = taskList.filter { !it.done }
        HomeUiState(
            timer = snapshot,
            nextTask = open.firstOrNull()?.title,
            openTasks = open.size,
            waterMl = water,
            waterGoalMl = prefs.waterGoalMl,
            habitsDone = habits.count { it.doneToday },
            habitsTotal = habits.size,
            screenTimeS = screen,
            mood = homeMood(snapshot.status, party, water, prefs.waterGoalMl, clock.timeOfDay()),
            look = prefs.look(),
        )
    }.stateIn(viewModelScope, WHILE_SUBSCRIBED, HomeUiState())

    /** Called when the screen comes back (usage access may have been granted meanwhile). */
    fun refresh() {
        viewModelScope.launch { screenTimeS.value = screenTime.today()?.totalS }
    }

    private companion object {
        const val CELEBRATE_MS = 5_000L
    }
}

data class FocusUiState(
    val timer: FocusTimer.Snapshot = FocusTimer.Snapshot.IDLE,
    val minutes: Int = 25,
    val look: MascotLook = AppSettings().look(),
)

@HiltViewModel
class FocusViewModel @Inject constructor(
    private val timer: FocusTimerController,
    private val settings: SettingsRepository,
) : ViewModel() {
    val state: StateFlow<FocusUiState> = combine(timer.state, settings.settings) { snapshot, prefs ->
        FocusUiState(snapshot, prefs.focusMinutes, prefs.look())
    }.stateIn(viewModelScope, WHILE_SUBSCRIBED, FocusUiState())

    fun setMinutes(minutes: Int) {
        val clamped = minutes.coerceIn(FocusTimer.MIN_MINUTES, FocusTimer.MAX_MINUTES)
        viewModelScope.launch { settings.update { it.copy(focusMinutes = clamped) } }
    }

    fun start(label: String) {
        timer.start(label, state.value.minutes)
    }

    fun pause() {
        timer.pause()
    }

    fun resume() {
        timer.resume()
    }

    fun stop() {
        timer.stop()
    }
}

@HiltViewModel
class TasksViewModel @Inject constructor(private val repository: TaskRepository) : ViewModel() {
    val tasks: StateFlow<List<Task>> = repository.tasks.stateIn(viewModelScope, WHILE_SUBSCRIBED, emptyList())

    /** Returns false (and adds nothing) for a blank title. */
    fun add(title: String): Boolean {
        if (title.isBlank()) return false
        viewModelScope.launch { repository.add(title) }
        return true
    }

    fun toggle(task: Task) {
        viewModelScope.launch { repository.setDone(task.id, !task.done) }
    }

    fun delete(task: Task) {
        viewModelScope.launch { repository.delete(task.id) }
    }
}

data class WellbeingUiState(
    val waterMl: Int = 0,
    val waterGoalMl: Int = 2000,
    val glassMl: Int = 250,
    val habits: List<Habit> = emptyList(),
    val screenTime: ScreenTimeToday? = null,
    val hasUsageAccess: Boolean = false,
    val celebrating: Boolean = false,
    val look: MascotLook = AppSettings().look(),
)

@HiltViewModel
class WellbeingViewModel @Inject constructor(
    private val repository: WellbeingRepository,
    private val settings: SettingsRepository,
    private val screenTime: ScreenTimeSource,
) : ViewModel() {
    private val screen = MutableStateFlow<ScreenTimeToday?>(null)
    private val access = MutableStateFlow(false)
    private val celebrating = MutableStateFlow(false)

    val state: StateFlow<WellbeingUiState> = combine(
        repository.waterToday(),
        repository.habits,
        settings.settings,
        combine(screen, access) { s, a -> s to a },
        celebrating,
    ) { water, habits, prefs, (today, granted), party ->
        WellbeingUiState(water, prefs.waterGoalMl, prefs.glassMl, habits, today, granted, party, prefs.look())
    }.stateIn(viewModelScope, WHILE_SUBSCRIBED, WellbeingUiState())

    fun addWater() {
        viewModelScope.launch { repository.addWater(state.value.glassMl) }
    }

    fun undoWater() {
        viewModelScope.launch { repository.undoWater() }
    }

    fun addHabit(name: String, target: Int): Boolean {
        if (name.isBlank() || target !in 1..20) return false
        viewModelScope.launch { repository.addHabit(name, target) }
        return true
    }

    fun logHabit(habit: Habit, delta: Int) {
        viewModelScope.launch {
            if (repository.logHabit(habit.id, delta)) {
                celebrating.value = true
                delay(4_000)
                celebrating.value = false
            }
        }
    }

    fun deleteHabit(habit: Habit) {
        viewModelScope.launch { repository.deleteHabit(habit.id) }
    }

    fun refreshScreenTime() {
        access.value = screenTime.hasAccess()
        viewModelScope.launch { screen.value = screenTime.today() }
    }
}

/** Starts/stops the floating island (a foreground service on Android). */
interface IslandController {
    fun start()
    fun stop()
}

data class SettingsUiState(
    val settings: AppSettings = AppSettings(),
    val permissions: PermissionState = PermissionState.NONE,
)

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val settings: SettingsRepository,
    private val permissionChecker: PermissionChecker,
    private val island: IslandController,
) : ViewModel() {
    private val permissions = MutableStateFlow(PermissionState.NONE)

    val state: StateFlow<SettingsUiState> = combine(settings.settings, permissions) { s, p ->
        SettingsUiState(s, p)
    }.stateIn(viewModelScope, WHILE_SUBSCRIBED, SettingsUiState())

    /** Whether saved settings are loaded yet (the activity waits, so onboarding doesn't flash). */
    val ready: StateFlow<Boolean> = settings.ready

    val onboardingDone: StateFlow<Boolean> = settings.settings.map { it.onboardingDone }
        .stateIn(viewModelScope, SharingStarted.Eagerly, false)

    /** Re-read permissions (call when returning from system settings). */
    fun refresh() {
        val now = permissionChecker.state()
        permissions.value = now
        // The island can't run without the overlay permission: turn it off instead of failing.
        if (settings.settings.value.islandEnabled) {
            if (now.overlay) island.start() else setIsland(false)
        }
    }

    fun setIsland(enabled: Boolean) {
        val allowed = enabled && permissionChecker.state().overlay
        viewModelScope.launch { settings.update { it.copy(islandEnabled = allowed) } }
        if (allowed) island.start() else island.stop()
    }

    fun setIslandSpot(spot: IslandSpot) {
        viewModelScope.launch { settings.update { it.copy(islandSpot = spot) } }
    }

    fun setMascot(style: MascotStyle? = null, color: String? = null) {
        viewModelScope.launch {
            settings.update {
                it.copy(
                    mascotStyle = style?.name?.lowercase() ?: it.mascotStyle,
                    mascotColor = color ?: it.mascotColor,
                )
            }
        }
    }

    fun setWaterGoal(ml: Int) {
        viewModelScope.launch { settings.update { it.copy(waterGoalMl = ml.coerceIn(500, 6000)) } }
    }

    fun finishOnboarding() {
        viewModelScope.launch { settings.update { it.copy(onboardingDone = true) } }
    }
}
