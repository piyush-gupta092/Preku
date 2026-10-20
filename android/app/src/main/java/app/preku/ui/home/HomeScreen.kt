package app.preku.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.hilt.navigation.compose.hiltViewModel
import app.preku.R
import app.preku.domain.FocusTimer
import app.preku.domain.formatClock
import app.preku.domain.formatDuration
import app.preku.ui.CardLabel
import app.preku.ui.Centered
import app.preku.ui.HomeViewModel
import app.preku.ui.MyPip
import app.preku.ui.PrekuCard
import app.preku.ui.theme.MonoTextStyle
import app.preku.ui.theme.PrekuTheme
import app.preku.ui.theme.Spacing
import java.time.LocalTime

/** Where the home cards lead. */
enum class HomeTarget { Focus, Tasks, Wellbeing, ScreenTimeSetup, Settings }

@Composable
fun HomeScreen(onOpen: (HomeTarget) -> Unit, viewModel: HomeViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(Unit) {
        viewModel.refresh()
        onPauseOrDispose { }
    }
    val colors = PrekuTheme.colors

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Spacing.s4, vertical = Spacing.s3),
        verticalArrangement = Arrangement.spacedBy(Spacing.s3),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = stringResource(greeting(LocalTime.now())),
                style = MaterialTheme.typography.displaySmall,
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = { onOpen(HomeTarget.Settings) }) {
                Icon(Icons.Filled.Settings, contentDescription = stringResource(R.string.open_settings))
            }
        }

        Centered {
            MyPip(state.mood, 200.dp, state.look, stringResource(R.string.pip_description))
        }

        PrekuCard(Modifier.fillMaxWidth(), accent = colors.modProductivity, onClick = { onOpen(HomeTarget.Focus) }) {
            CardLabel(stringResource(R.string.card_focus), colors.modProductivity)
            val t = state.timer
            Text(
                text = when (t.status) {
                    FocusTimer.Status.Idle -> stringResource(R.string.home_focus_idle)
                    FocusTimer.Status.Running -> stringResource(R.string.home_focusing, formatClock(t.remainingS))
                    FocusTimer.Status.Paused -> stringResource(R.string.home_paused, formatClock(t.remainingS))
                },
                style = if (t.status == FocusTimer.Status.Idle) MaterialTheme.typography.titleMedium else MonoTextStyle,
            )
            if (t.status != FocusTimer.Status.Idle) {
                LinearProgressIndicator(progress = { t.progress }, color = colors.modProductivity, modifier = Modifier.fillMaxWidth())
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s3)) {
            PrekuCard(Modifier.weight(1f), accent = colors.modProductivity, onClick = { onOpen(HomeTarget.Tasks) }) {
                CardLabel(stringResource(R.string.card_tasks), colors.modProductivity)
                Text(
                    text = state.nextTask ?: stringResource(R.string.home_tasks_none),
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                if (state.openTasks > 0) {
                    Text(
                        pluralStringResource(R.plurals.home_tasks_open, state.openTasks, state.openTasks),
                        style = MaterialTheme.typography.labelSmall,
                        color = colors.textMuted,
                    )
                }
            }
            PrekuCard(Modifier.weight(1f), accent = colors.modWellbeing, onClick = { onOpen(HomeTarget.Wellbeing) }) {
                CardLabel(stringResource(R.string.card_water), colors.modWellbeing)
                Text(stringResource(R.string.home_water, state.waterMl, state.waterGoalMl), style = MaterialTheme.typography.titleMedium)
                LinearProgressIndicator(
                    progress = { state.waterMl.toFloat() / state.waterGoalMl },
                    color = colors.modWellbeing,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s3)) {
            PrekuCard(Modifier.weight(1f), accent = colors.modWellbeing, onClick = { onOpen(HomeTarget.Wellbeing) }) {
                CardLabel(stringResource(R.string.card_habits), colors.modWellbeing)
                Text(
                    text = if (state.habitsTotal == 0) {
                        stringResource(R.string.home_habits_none)
                    } else {
                        pluralStringResource(R.plurals.home_habits, state.habitsTotal, state.habitsDone, state.habitsTotal)
                    },
                    style = MaterialTheme.typography.titleMedium,
                )
            }
            PrekuCard(
                Modifier.weight(1f),
                accent = colors.modMonitor,
                onClick = { onOpen(if (state.screenTimeS == null) HomeTarget.ScreenTimeSetup else HomeTarget.Wellbeing) },
            ) {
                CardLabel(stringResource(R.string.card_screen), colors.modMonitor)
                Text(
                    text = state.screenTimeS?.let { stringResource(R.string.home_screen_time, formatDuration(it)) }
                        ?: stringResource(R.string.home_screen_setup),
                    style = MaterialTheme.typography.titleMedium,
                )
            }
        }
    }
}

private fun greeting(time: LocalTime): Int = when (time.hour) {
    in 5..11 -> R.string.greeting_morning
    in 12..17 -> R.string.greeting_afternoon
    in 18..22 -> R.string.greeting_evening
    else -> R.string.greeting_night
}
