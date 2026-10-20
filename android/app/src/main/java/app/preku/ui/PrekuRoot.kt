package app.preku.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.preku.R
import app.preku.platform.PermissionIntents
import app.preku.ui.bridge.BridgeViewModel
import app.preku.ui.focus.FocusScreen
import app.preku.ui.home.HomeScreen
import app.preku.ui.home.HomeTarget
import app.preku.ui.onboarding.OnboardingScreen
import app.preku.ui.settings.SettingsScreen
import app.preku.ui.tasks.TasksScreen
import app.preku.ui.wellbeing.WellbeingScreen

enum class Tab(val label: Int, val icon: ImageVector) {
    Home(R.string.nav_home, Icons.Filled.Home),
    Focus(R.string.nav_focus, Icons.Filled.PlayArrow),
    Tasks(R.string.nav_tasks, Icons.AutoMirrored.Filled.List),
    Wellbeing(R.string.nav_wellbeing, Icons.Filled.Favorite),
}

/** The app: onboarding on first run, then four tabs (and Settings on top). */
@Composable
fun PrekuRoot(settingsViewModel: SettingsViewModel = hiltViewModel()) {
    val ready by settingsViewModel.ready.collectAsStateWithLifecycle()
    val onboarded by settingsViewModel.onboardingDone.collectAsStateWithLifecycle()

    Surface(color = MaterialTheme.colorScheme.background, modifier = Modifier.fillMaxSize()) {
        when {
            // Wait for saved settings, so onboarding doesn't flash for returning users.
            !ready -> Box(Modifier.fillMaxSize())
            !onboarded -> Scaffold { padding -> Box(Modifier.padding(padding)) { OnboardingScreen(settingsViewModel) } }
            else -> MainTabs()
        }
    }
}

@Composable
private fun MainTabs(bridge: BridgeViewModel = hiltViewModel()) {
    // A paired PC stays connected in the background (BridgeService).
    val linked by bridge.state.collectAsStateWithLifecycle()
    LaunchedEffect(linked.pc != null) { bridge.ensureService() }
    var tab by rememberSaveable { mutableStateOf(Tab.Home) }
    var settingsOpen by rememberSaveable { mutableStateOf(false) }
    val context = LocalContext.current

    BackHandler(enabled = settingsOpen || tab != Tab.Home) {
        if (settingsOpen) settingsOpen = false else tab = Tab.Home
    }

    Scaffold(
        bottomBar = {
            if (!settingsOpen) {
                NavigationBar {
                    Tab.entries.forEach { item ->
                        NavigationBarItem(
                            selected = tab == item,
                            onClick = { tab = item },
                            icon = { Icon(item.icon, contentDescription = null) },
                            label = { Text(stringResource(item.label)) },
                        )
                    }
                }
            }
        },
    ) { padding ->
        Box(Modifier.padding(padding)) {
            if (settingsOpen) {
                SettingsScreen(onBack = { settingsOpen = false })
            } else {
                when (tab) {
                    Tab.Home -> HomeScreen(onOpen = { target ->
                        when (target) {
                            HomeTarget.Focus -> tab = Tab.Focus
                            HomeTarget.Tasks -> tab = Tab.Tasks
                            HomeTarget.Wellbeing -> tab = Tab.Wellbeing
                            HomeTarget.Settings -> settingsOpen = true
                            HomeTarget.ScreenTimeSetup -> context.startActivity(PermissionIntents.usageAccess())
                        }
                    })
                    Tab.Focus -> FocusScreen()
                    Tab.Tasks -> TasksScreen()
                    Tab.Wellbeing -> WellbeingScreen()
                }
            }
        }
    }
}
