package app.preku.ui

import app.preku.data.AppSettings
import app.preku.data.FocusTimerController
import app.preku.data.Habit
import app.preku.data.SettingsRepository
import app.preku.data.Task
import app.preku.data.TaskRepository
import app.preku.data.TimerStorage
import app.preku.data.WellbeingRepository
import app.preku.domain.Clock
import app.preku.domain.FocusTimer
import app.preku.domain.TimerSave
import app.preku.pip.PipMood
import app.preku.platform.PermissionChecker
import app.preku.platform.PermissionState
import app.preku.platform.ScreenTimeSource
import app.preku.platform.ScreenTimeToday
import java.time.LocalDate
import java.time.LocalTime
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

// ---- Fakes ----

class FakeClock(var ms: Long = 0, var time: LocalTime = LocalTime.NOON) : Clock {
    override fun now() = ms
    override fun today(): LocalDate = LocalDate.of(2026, 10, 10)
    override fun timeOfDay() = time
}

class FakeTasks : TaskRepository {
    val list = MutableStateFlow<List<Task>>(emptyList())
    override val tasks: Flow<List<Task>> = list
    override suspend fun add(title: String) {
        list.value = list.value + Task("t${list.value.size}", title.trim(), false)
    }
    override suspend fun setDone(id: String, done: Boolean) {
        list.value = list.value.map { if (it.id == id) it.copy(done = done) else it }
    }
    override suspend fun delete(id: String) {
        list.value = list.value.filterNot { it.id == id }
    }
}

class FakeWellbeing : WellbeingRepository {
    val water = MutableStateFlow(0)
    val habitList = MutableStateFlow<List<Habit>>(emptyList())
    var milestone = false
    override fun waterToday(): Flow<Int> = water
    override suspend fun addWater(ml: Int) { water.value += ml }
    override suspend fun undoWater() { water.value = 0 }
    override val habits: Flow<List<Habit>> = habitList
    override suspend fun addHabit(name: String, target: Int) {
        habitList.value = habitList.value + Habit(name, name, target, 0, false, 0)
    }
    override suspend fun deleteHabit(id: String) = Unit
    override suspend fun logHabit(id: String, delta: Int) = milestone
}

class FakeSettings(initial: AppSettings = AppSettings()) : SettingsRepository {
    private val state = MutableStateFlow(initial)
    override val settings: StateFlow<AppSettings> = state
    override val ready: StateFlow<Boolean> = MutableStateFlow(true)
    override suspend fun update(transform: (AppSettings) -> AppSettings) { state.value = transform(state.value) }
}

class FakeScreenTime(var access: Boolean = false) : ScreenTimeSource {
    override fun hasAccess() = access
    override suspend fun today() = if (access) ScreenTimeToday(3600, emptyList()) else null
}

class FakeStorage(var saved: TimerSave? = null) : TimerStorage {
    val records = mutableListOf<FocusTimer.SessionRecord>()
    override suspend fun load() = saved
    override suspend fun save(save: TimerSave?) { saved = save }
    override suspend fun record(session: FocusTimer.SessionRecord) { records += session }
}

class FakePermissions(var state: PermissionState = PermissionState.NONE) : PermissionChecker {
    override fun state() = state
}

class FakeIsland : IslandController {
    var running = false
    override fun start() { running = true }
    override fun stop() { running = false }
}

@OptIn(ExperimentalCoroutinesApi::class)
class ViewModelTest {
    private val dispatcher = StandardTestDispatcher()

    @Before
    fun setUp() = Dispatchers.setMain(dispatcher)

    @After
    fun tearDown() = Dispatchers.resetMain()

    @Test
    fun pipsHomeMoodFollowsTheDay() {
        val noon = LocalTime.NOON
        assertEquals(PipMood.Idle, homeMood(FocusTimer.Status.Idle, false, 0, 2000, noon))
        assertEquals(PipMood.Focus, homeMood(FocusTimer.Status.Running, false, 0, 2000, noon))
        assertEquals(PipMood.Celebrate, homeMood(FocusTimer.Status.Running, true, 0, 2000, noon))
        assertEquals(PipMood.Happy, homeMood(FocusTimer.Status.Idle, false, 2000, 2000, noon))
        assertEquals(PipMood.Sleepy, homeMood(FocusTimer.Status.Idle, false, 0, 2000, LocalTime.of(23, 30)))
    }

    @Test
    fun tasksIgnoreBlankTitles() = runTest(dispatcher) {
        val repo = FakeTasks()
        val vm = TasksViewModel(repo)
        assertFalse(vm.add("   "))
        assertTrue(vm.add("  Buy milk "))
        advanceUntilIdle()
        assertEquals(listOf("Buy milk"), repo.list.value.map { it.title })
        vm.toggle(repo.list.value.first())
        advanceUntilIdle()
        assertTrue(repo.list.value.first().done)
    }

    private fun TestScope.timer(clock: FakeClock, storage: FakeStorage, notified: MutableList<String>) =
        FocusTimerController(storage, { notified += it }, clock, backgroundScope)

    @Test
    fun timerCountsDownCompletesAndNotifies() = runTest(dispatcher) {
        val clock = FakeClock()
        val storage = FakeStorage()
        val notified = mutableListOf<String>()
        val controller = timer(clock, storage, notified)
        runCurrent()

        controller.start("Write", 1)
        runCurrent()
        assertEquals(FocusTimer.Status.Running, controller.state.value.status)
        assertTrue("saved for restarts", storage.saved != null)

        clock.ms = 30_000
        advanceTimeBy(1_001)
        assertEquals(30, controller.state.value.remainingS)

        clock.ms = 60_000
        advanceTimeBy(1_001)
        assertEquals(FocusTimer.Status.Idle, controller.state.value.status)
        assertEquals(listOf("Write"), notified)
        assertTrue(storage.records.single().completed)
        assertEquals(null, storage.saved)
    }

    @Test
    fun aTimerThatRanOutWhileClosedCompletesOnStart() = runTest(dispatcher) {
        val clock = FakeClock(ms = 10 * 60_000)
        val storage = FakeStorage(TimerSave("Old", 60_000, 0, 0, segmentStart = 0))
        val notified = mutableListOf<String>()
        val controller = timer(clock, storage, notified)
        runCurrent()
        assertEquals(FocusTimer.Status.Idle, controller.state.value.status)
        assertEquals(listOf("Old"), notified)
    }

    @Test
    fun focusScreenUsesTheSavedLength() = runTest(dispatcher) {
        val clock = FakeClock()
        val settings = FakeSettings(AppSettings(focusMinutes = 45))
        val controller = timer(clock, FakeStorage(), mutableListOf())
        val vm = FocusViewModel(controller, settings)
        backgroundScope.launch { vm.state.collect {} }
        runCurrent()
        vm.start("Read")
        runCurrent()
        assertEquals(45 * 60, controller.state.value.totalS)
        vm.setMinutes(500)
        advanceUntilIdle()
        assertEquals(FocusTimer.MAX_MINUTES, settings.settings.value.focusMinutes)
    }

    @Test
    fun waterUsesTheGlassSizeAndMilestonesCelebrate() = runTest(dispatcher) {
        val repo = FakeWellbeing()
        val vm = WellbeingViewModel(repo, FakeSettings(AppSettings(glassMl = 300)), FakeScreenTime())
        backgroundScope.launch { vm.state.collect {} }
        runCurrent()
        vm.addWater()
        runCurrent()
        assertEquals(300, vm.state.value.waterMl)

        repo.milestone = true
        vm.logHabit(Habit("h", "Read", 1, 0, false, 6), 1)
        runCurrent()
        assertTrue(vm.state.value.celebrating)
        assertFalse(vm.addHabit(" ", 1))
    }

    @Test
    fun screenTimeDegradesWithoutUsageAccess() = runTest(dispatcher) {
        val screen = FakeScreenTime(access = false)
        val vm = WellbeingViewModel(FakeWellbeing(), FakeSettings(), screen)
        backgroundScope.launch { vm.state.collect {} }
        vm.refreshScreenTime()
        runCurrent()
        assertFalse(vm.state.value.hasUsageAccess)
        assertEquals(null, vm.state.value.screenTime)

        screen.access = true
        vm.refreshScreenTime()
        runCurrent()
        assertEquals(3600L, vm.state.value.screenTime?.totalS)
    }

    @Test
    fun theIslandNeedsTheOverlayPermission() = runTest(dispatcher) {
        val permissions = FakePermissions()
        val island = FakeIsland()
        val settings = FakeSettings()
        val vm = SettingsViewModel(settings, permissions, island)

        vm.setIsland(true)
        advanceUntilIdle()
        assertFalse("denied: stays off", settings.settings.value.islandEnabled)
        assertFalse(island.running)

        permissions.state = PermissionState.NONE.copy(overlay = true)
        vm.setIsland(true)
        advanceUntilIdle()
        assertTrue(settings.settings.value.islandEnabled)
        assertTrue(island.running)

        // Permission revoked in system settings: refresh turns the island off cleanly.
        permissions.state = PermissionState.NONE
        vm.refresh()
        advanceUntilIdle()
        assertFalse(settings.settings.value.islandEnabled)
        assertFalse(island.running)
    }

    @Test
    fun homeSummarisesTheDay() = runTest(dispatcher) {
        val tasks = FakeTasks().apply { list.value = listOf(Task("1", "Email Sam", false), Task("2", "Done", true)) }
        val wellbeing = FakeWellbeing().apply { water.value = 500 }
        val clock = FakeClock()
        val vm = HomeViewModel(timer(clock, FakeStorage(), mutableListOf()), tasks, wellbeing, FakeSettings(), FakeScreenTime(), clock)
        backgroundScope.launch { vm.state.collect {} }
        runCurrent()
        val state = vm.state.map { it }.first { it.openTasks > 0 }
        assertEquals("Email Sam", state.nextTask)
        assertEquals(1, state.openTasks)
        assertEquals(500, state.waterMl)
        assertEquals(null, state.screenTimeS)
    }
}
