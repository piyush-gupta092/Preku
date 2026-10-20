package app.preku.ui.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.preku.R
import app.preku.pip.PipMood
import app.preku.ui.MyPip
import app.preku.ui.SettingsViewModel
import app.preku.ui.look
import app.preku.ui.settings.PermissionList
import app.preku.ui.theme.PrekuTheme
import app.preku.ui.theme.Spacing

/**
 * First run: Pip says hi and explains each permission. All of them are optional; "Let's go"
 * works with none granted (every feature degrades gracefully).
 */
@Composable
fun OnboardingScreen(viewModel: SettingsViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(Unit) {
        viewModel.refresh()
        onPauseOrDispose { }
    }
    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(Spacing.s4),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(Spacing.s3),
    ) {
        MyPip(PipMood.Happy, 180.dp, state.settings.look(), stringResource(R.string.pip_description))
        Text(stringResource(R.string.onboarding_title), style = MaterialTheme.typography.displaySmall)
        Text(
            stringResource(R.string.onboarding_text),
            style = MaterialTheme.typography.bodyMedium,
            color = PrekuTheme.colors.textMuted,
            textAlign = TextAlign.Center,
        )
        PermissionList(state.permissions)
        Button(onClick = viewModel::finishOnboarding, modifier = Modifier.fillMaxWidth()) {
            Text(stringResource(R.string.onboarding_continue))
        }
    }
}
