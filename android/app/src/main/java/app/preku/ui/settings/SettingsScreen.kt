package app.preku.ui.settings

import android.Manifest
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.preku.BuildConfig
import app.preku.R
import app.preku.data.IslandSpot
import app.preku.pip.MASCOT_COLORS
import app.preku.pip.MascotStyle
import app.preku.pip.PipMood
import app.preku.platform.PermissionIntents
import app.preku.platform.PermissionState
import app.preku.ui.MyPip
import app.preku.ui.PrekuCard
import app.preku.ui.SectionTitle
import app.preku.ui.bridge.PcSection
import app.preku.ui.SettingsViewModel
import app.preku.ui.look
import app.preku.ui.theme.PrekuTheme
import app.preku.ui.theme.Spacing

private val STYLE_NAMES = mapOf(
    MascotStyle.Classic to R.string.style_classic,
    MascotStyle.Sprout to R.string.style_sprout,
    MascotStyle.Ghost to R.string.style_ghost,
    MascotStyle.Chubby to R.string.style_chubby,
)

private val COLOR_NAMES = mapOf(
    "peach" to R.string.color_peach, "lemon" to R.string.color_lemon, "mint" to R.string.color_mint,
    "sky" to R.string.color_sky, "lavender" to R.string.color_lavender, "bubblegum" to R.string.color_bubblegum,
    "cocoa" to R.string.color_cocoa, "snow" to R.string.color_snow,
)

@Composable
fun SettingsScreen(onBack: () -> Unit, viewModel: SettingsViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(Unit) {
        viewModel.refresh()
        onPauseOrDispose { }
    }
    val settings = state.settings
    val look = settings.look()

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(Spacing.s4),
        verticalArrangement = Arrangement.spacedBy(Spacing.s2),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back))
            }
            Text(stringResource(R.string.settings_title), style = MaterialTheme.typography.displaySmall)
        }

        // ---- Your Pip ----
        SectionTitle(stringResource(R.string.settings_pip))
        Text(stringResource(R.string.settings_pip_hint), style = MaterialTheme.typography.bodyMedium, color = PrekuTheme.colors.textMuted)
        Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s2), modifier = Modifier.fillMaxWidth()) {
            MascotStyle.entries.forEach { style ->
                val selected = look.style == style
                Surface(
                    shape = MaterialTheme.shapes.medium,
                    border = BorderStroke(if (selected) 2.dp else 1.dp, if (selected) MaterialTheme.colorScheme.primary else PrekuTheme.colors.border),
                    modifier = Modifier.weight(1f).clickable { viewModel.setMascot(style = style) },
                ) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.padding(vertical = Spacing.s2)) {
                        MyPip(if (selected) PipMood.Happy else PipMood.Idle, 64.dp, look.copy(style = style), stringResource(STYLE_NAMES.getValue(style)))
                        Text(stringResource(STYLE_NAMES.getValue(style)), style = MaterialTheme.typography.labelMedium)
                    }
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s2)) {
            MASCOT_COLORS.forEach { (id, hex) ->
                val selected = settings.mascotColor.equals(hex, ignoreCase = true)
                val name = stringResource(COLOR_NAMES.getValue(id))
                Box(
                    contentAlignment = Alignment.Center,
                    modifier = Modifier
                        .size(32.dp)
                        .background(Color(android.graphics.Color.parseColor(hex)), CircleShape)
                        .border(if (selected) 3.dp else 1.dp, if (selected) MaterialTheme.colorScheme.primary else PrekuTheme.colors.border, CircleShape)
                        .clickable { viewModel.setMascot(color = hex) }
                        .semantics { contentDescription = name },
                ) {
                    if (selected) Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(16.dp), tint = Color(0xFF14141A))
                }
            }
        }

        // ---- Floating island ----
        SectionTitle(stringResource(R.string.settings_island))
        PrekuCard(Modifier.fillMaxWidth()) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.settings_island_hint), style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                Switch(
                    checked = settings.islandEnabled,
                    onCheckedChange = { viewModel.setIsland(it) },
                    enabled = state.permissions.overlay,
                )
            }
            if (!state.permissions.overlay) {
                Text(stringResource(R.string.settings_island_needs), style = MaterialTheme.typography.labelSmall, color = PrekuTheme.colors.warning)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s2)) {
                listOf(IslandSpot.Left to R.string.spot_left, IslandSpot.Center to R.string.spot_center, IslandSpot.Right to R.string.spot_right)
                    .forEach { (spot, label) ->
                        FilterChip(
                            selected = settings.islandSpot == spot,
                            onClick = { viewModel.setIslandSpot(spot) },
                            label = { Text(stringResource(label)) },
                        )
                    }
            }
        }

        // ---- Your PC (bridge) ----
        PcSection()

        // ---- Water goal ----
        SectionTitle(stringResource(R.string.settings_water_goal))
        Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s2)) {
            listOf(1500, 2000, 2500, 3000).forEach { ml ->
                FilterChip(
                    selected = settings.waterGoalMl == ml,
                    onClick = { viewModel.setWaterGoal(ml) },
                    label = { Text("$ml ml") },
                )
            }
        }

        // ---- Permissions ----
        SectionTitle(stringResource(R.string.settings_permissions))
        PermissionList(state.permissions)

        Text(stringResource(R.string.settings_privacy), style = MaterialTheme.typography.bodyMedium, color = PrekuTheme.colors.textMuted, modifier = Modifier.padding(top = Spacing.s4))
        Text(stringResource(R.string.version, BuildConfig.VERSION_NAME), style = MaterialTheme.typography.labelSmall, color = PrekuTheme.colors.textMuted)
    }
}

/** One row per permission: what it's for, and Allow (or a tick when granted). */
@Composable
fun PermissionList(permissions: PermissionState) {
    val context = LocalContext.current
    val notificationLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
    Column(verticalArrangement = Arrangement.spacedBy(Spacing.s2)) {
        PermissionRow(R.string.perm_notifications, R.string.perm_notifications_why, permissions.notifications) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                notificationLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
            } else {
                context.startActivity(PermissionIntents.appNotifications(context))
            }
        }
        PermissionRow(R.string.perm_overlay, R.string.perm_overlay_why, permissions.overlay) {
            context.startActivity(PermissionIntents.overlay(context))
        }
        PermissionRow(R.string.perm_usage, R.string.perm_usage_why, permissions.usageAccess) {
            context.startActivity(PermissionIntents.usageAccess())
        }
        PermissionRow(R.string.perm_battery, R.string.perm_battery_why, permissions.batteryUnrestricted) {
            context.startActivity(PermissionIntents.battery())
        }
    }
}

@Composable
private fun PermissionRow(title: Int, why: Int, granted: Boolean, onAllow: () -> Unit) {
    PrekuCard(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(title), style = MaterialTheme.typography.titleMedium)
                Text(stringResource(why), style = MaterialTheme.typography.bodyMedium, color = PrekuTheme.colors.textMuted)
            }
            if (granted) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Filled.Check, contentDescription = null, tint = PrekuTheme.colors.success)
                    Text(stringResource(R.string.perm_granted), style = MaterialTheme.typography.labelMedium, color = PrekuTheme.colors.success)
                }
            } else {
                OutlinedButton(onClick = onAllow) { Text(stringResource(R.string.perm_allow)) }
            }
        }
    }
}
