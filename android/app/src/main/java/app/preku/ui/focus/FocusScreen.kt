package app.preku.ui.focus

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.preku.R
import app.preku.domain.FocusTimer
import app.preku.domain.formatClock
import app.preku.pip.PipMood
import app.preku.ui.FocusViewModel
import app.preku.ui.MyPip
import app.preku.ui.ProgressRing
import app.preku.ui.theme.MonoFont
import app.preku.ui.theme.PrekuTheme
import app.preku.ui.theme.Spacing

private val PRESETS = listOf(15, 25, 45, 60)

@Composable
fun FocusScreen(viewModel: FocusViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var label by rememberSaveable { mutableStateOf("") }
    val timer = state.timer
    val accent = PrekuTheme.colors.modProductivity
    val idle = timer.status == FocusTimer.Status.Idle

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(Spacing.s4),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(Spacing.s4),
    ) {
        ProgressRing(progress = timer.progress, size = 260.dp, color = accent, stroke = 10.dp) {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                MyPip(
                    if (timer.status == FocusTimer.Status.Running) PipMood.Focus else PipMood.Idle,
                    110.dp,
                    state.look,
                    stringResource(R.string.pip_description),
                )
                Text(
                    text = if (idle) formatClock(state.minutes * 60) else formatClock(timer.remainingS),
                    style = TextStyle(fontFamily = MonoFont, fontSize = 40.sp, fontFeatureSettings = "tnum"),
                )
                Text(
                    text = if (idle) stringResource(R.string.focus_ready) else timer.label,
                    style = MaterialTheme.typography.bodyMedium,
                    color = PrekuTheme.colors.textMuted,
                )
            }
        }

        if (idle) {
            OutlinedTextField(
                value = label,
                onValueChange = { label = it.take(60) },
                placeholder = { Text(stringResource(R.string.focus_label_hint)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s2)) {
                PRESETS.forEach { minutes ->
                    FilterChip(
                        selected = state.minutes == minutes,
                        onClick = { viewModel.setMinutes(minutes) },
                        label = { Text(stringResource(R.string.focus_minutes, minutes)) },
                    )
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s2), verticalAlignment = Alignment.CenterVertically) {
                OutlinedButton(onClick = { viewModel.setMinutes(state.minutes - 5) }) { Text(stringResource(R.string.focus_less)) }
                Text(stringResource(R.string.focus_minutes, state.minutes), style = MaterialTheme.typography.titleMedium)
                OutlinedButton(onClick = { viewModel.setMinutes(state.minutes + 5) }) { Text(stringResource(R.string.focus_more)) }
            }
            Button(
                onClick = { viewModel.start(label) },
                colors = ButtonDefaults.buttonColors(containerColor = accent),
                modifier = Modifier.fillMaxWidth(),
            ) { Text(stringResource(R.string.focus_start)) }
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(Spacing.s3)) {
                if (timer.status == FocusTimer.Status.Running) {
                    Button(onClick = viewModel::pause) { Text(stringResource(R.string.focus_pause)) }
                } else {
                    Button(onClick = viewModel::resume) { Text(stringResource(R.string.focus_resume)) }
                }
                OutlinedButton(onClick = viewModel::stop) { Text(stringResource(R.string.focus_stop)) }
            }
        }
    }
}
