package app.preku.data

import app.preku.di.ApplicationScope
import app.preku.domain.Clock
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** Island position along the top edge (the overlay snaps to these). */
enum class IslandSpot { Left, Center, Right }

/** The phone app's preferences: one JSON document under settings key `app`. */
@Serializable
data class AppSettings(
    val onboardingDone: Boolean = false,
    val waterGoalMl: Int = 2000,
    val glassMl: Int = 250,
    val focusMinutes: Int = 25,
    /** classic | sprout | ghost | chubby (same bodies as the desktop). */
    val mascotStyle: String = "classic",
    val mascotColor: String = "#ffb38a",
    /** The floating island overlay (needs the "display over other apps" permission). */
    val islandEnabled: Boolean = false,
    val islandSpot: IslandSpot = IslandSpot.Center,
)

interface SettingsRepository {
    val settings: StateFlow<AppSettings>

    /** False until the saved settings are loaded (so onboarding doesn't flash). */
    val ready: StateFlow<Boolean>
    suspend fun update(transform: (AppSettings) -> AppSettings)
}

@Singleton
class RoomSettingsRepository @Inject constructor(
    private val dao: SettingDao,
    private val clock: Clock,
    @ApplicationScope scope: CoroutineScope,
) : SettingsRepository {
    private val state = MutableStateFlow(AppSettings())
    override val settings: StateFlow<AppSettings> = state.asStateFlow()
    private val loaded = MutableStateFlow(false)
    override val ready: StateFlow<Boolean> = loaded.asStateFlow()

    init {
        scope.launch {
            dao.get(KEY)?.let { json ->
                // A corrupt document falls back to defaults instead of crashing.
                runCatching { JSON.decodeFromString(AppSettings.serializer(), json) }.onSuccess { state.value = it }
            }
            loaded.value = true
        }
    }

    override suspend fun update(transform: (AppSettings) -> AppSettings) {
        val next = transform(state.value)
        state.value = next
        dao.upsert(SettingEntity(KEY, JSON.encodeToString(AppSettings.serializer(), next), clock.now()))
    }

    private companion object {
        const val KEY = "app"
        val JSON = Json { ignoreUnknownKeys = true }
    }
}
