package app.preku.ui.wellbeing

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.preku.R
import app.preku.data.Habit
import app.preku.domain.formatDuration
import app.preku.pip.PipMood
import app.preku.platform.PermissionIntents
import app.preku.ui.MyPip
import app.preku.ui.PrekuCard
import app.preku.ui.ProgressRing
import app.preku.ui.SectionTitle
import app.preku.ui.WellbeingViewModel
import app.preku.ui.theme.MonoTextStyle
import app.preku.ui.theme.PrekuTheme
import app.preku.ui.theme.Spacing

@Composable
fun WellbeingScreen(viewModel: WellbeingViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(Unit) {
        viewModel.refreshScreenTime()
        onPauseOrDispose { }
    }
    val accent = PrekuTheme.colors.modWellbeing
    val context = LocalContext.current
    var habitName by rememberSaveable { mutableStateOf("") }

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(Spacing.s4),
        verticalArrangement = Arrangement.spacedBy(Spacing.s2),
    ) {
        // ---- Water ----
        PrekuCard(Modifier.fillMaxWidth(), accent = accent) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.s4)) {
                ProgressRing(state.waterMl.toFloat() / state.waterGoalMl, 120.dp, accent) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Text("${state.waterMl}", style = MonoTextStyle)
                        Text(
                            stringResource(R.string.water_of, state.waterGoalMl),
                            style = MaterialTheme.typography.labelSmall,
                            color = PrekuTheme.colors.textMuted,
                        )
                    }
                }
                Column(verticalArrangement = Arrangement.spacedBy(Spacing.s2), modifier = Modifier.weight(1f)) {
                    MyPip(
                        if (state.celebrating) PipMood.Celebrate else PipMood.Sip,
                        72.dp,
                        state.look,
                        stringResource(R.string.pip_description),
                    )
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Button(onClick = viewModel::addWater, colors = ButtonDefaults.buttonColors(containerColor = accent)) {
                            Text(stringResource(R.string.water_add, state.glassMl))
                        }
                        IconButton(onClick = viewModel::undoWater, enabled = state.waterMl > 0) {
                            Icon(Icons.Filled.Refresh, contentDescription = stringResource(R.string.water_undo))
                        }
                    }
                }
            }
        }

        // ---- Habits ----
        SectionTitle(stringResource(R.string.habits_title))
        if (state.habits.isEmpty()) {
            Text(stringResource(R.string.habits_empty), style = MaterialTheme.typography.bodyMedium, color = PrekuTheme.colors.textMuted)
        }
        state.habits.forEach { habit -> HabitRow(habit, viewModel) }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.s2)) {
            OutlinedTextField(
                value = habitName,
                onValueChange = { habitName = it.take(80) },
                placeholder = { Text(stringResource(R.string.habit_hint)) },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            FilledIconButton(onClick = { if (viewModel.addHabit(habitName, 1)) habitName = "" }) {
                Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.habit_add))
            }
        }

        // ---- Screen time ----
        SectionTitle(stringResource(R.string.screen_title))
        val today = state.screenTime
        if (!state.hasUsageAccess || today == null) {
            PrekuCard(Modifier.fillMaxWidth(), accent = PrekuTheme.colors.modMonitor) {
                Text(stringResource(R.string.screen_needs_access), style = MaterialTheme.typography.bodyMedium)
                OutlinedButton(onClick = { context.startActivity(PermissionIntents.usageAccess()) }) {
                    Text(stringResource(R.string.screen_grant))
                }
            }
        } else if (today.apps.isEmpty()) {
            Text(stringResource(R.string.screen_empty), style = MaterialTheme.typography.bodyMedium, color = PrekuTheme.colors.textMuted)
        } else {
            Text(formatDuration(today.totalS), style = MaterialTheme.typography.displaySmall)
            val top = today.apps.first().seconds.coerceAtLeast(1)
            today.apps.forEach { app ->
                Column(Modifier.fillMaxWidth()) {
                    Row {
                        Text(app.label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                        Text(formatDuration(app.seconds), style = MonoTextStyle, color = PrekuTheme.colors.textMuted)
                    }
                    LinearProgressIndicator(
                        progress = { app.seconds.toFloat() / top },
                        color = PrekuTheme.colors.modMonitor,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
    }
}

@Composable
private fun HabitRow(habit: Habit, viewModel: WellbeingViewModel) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        if (habit.target == 1) {
            val checkIn = stringResource(R.string.habit_check, habit.name)
            Checkbox(
                checked = habit.doneToday,
                onCheckedChange = { viewModel.logHabit(habit, if (it) 1 else -1) },
                modifier = Modifier.semantics { contentDescription = checkIn },
            )
        } else {
            Row(verticalAlignment = Alignment.CenterVertically) {
                val less = stringResource(R.string.habit_less)
                val more = stringResource(R.string.habit_more)
                OutlinedButton(
                    onClick = { viewModel.logHabit(habit, -1) },
                    modifier = Modifier.semantics { contentDescription = less },
                ) { Text("−") }
                Text(" ${habit.countToday}/${habit.target} ", style = MonoTextStyle)
                OutlinedButton(
                    onClick = { viewModel.logHabit(habit, 1) },
                    modifier = Modifier.semantics { contentDescription = more },
                ) { Text("+") }
            }
        }
        Column(Modifier.weight(1f).padding(start = Spacing.s2)) {
            Text(habit.name, style = MaterialTheme.typography.titleMedium)
            if (habit.streak > 0) {
                Text(
                    "🔥 " + pluralStringResource(R.plurals.habit_streak, habit.streak, habit.streak),
                    style = MaterialTheme.typography.labelSmall,
                    color = PrekuTheme.colors.textMuted,
                )
            }
        }
        IconButton(onClick = { viewModel.deleteHabit(habit) }) {
            Icon(Icons.Filled.Delete, contentDescription = stringResource(R.string.habit_delete))
        }
    }
}
