package app.preku.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.preku.pip.Pip
import app.preku.pip.PipMood
import app.preku.ui.theme.PrekuTheme
import app.preku.ui.theme.Spacing

/** A rounded card in the app's surface colors; clickable when [onClick] is set. */
@Composable
fun PrekuCard(
    modifier: Modifier = Modifier,
    accent: Color? = null,
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    Surface(
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, accent?.copy(alpha = 0.35f) ?: PrekuTheme.colors.border),
        modifier = modifier.then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier),
    ) {
        Column(Modifier.padding(Spacing.s4), verticalArrangement = Arrangement.spacedBy(Spacing.s1), content = content)
    }
}

@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier) {
    Text(
        text = text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = PrekuTheme.colors.textMuted,
        modifier = modifier.padding(top = Spacing.s4, bottom = Spacing.s2),
    )
}

@Composable
fun CardLabel(text: String, accent: Color) {
    Text(text, style = MaterialTheme.typography.labelMedium, color = accent)
}

/** A progress ring (0…1) with content in the middle. */
@Composable
fun ProgressRing(
    progress: Float,
    size: Dp,
    color: Color,
    modifier: Modifier = Modifier,
    stroke: Dp = 8.dp,
    content: @Composable () -> Unit = {},
) {
    val track = MaterialTheme.colorScheme.surfaceVariant
    Box(modifier.size(size), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(size)) {
            val width = stroke.toPx()
            drawArc(track, 0f, 360f, useCenter = false, style = Stroke(width))
            drawArc(
                color,
                -90f,
                360f * progress.coerceIn(0f, 1f),
                useCenter = false,
                style = Stroke(width, cap = StrokeCap.Round),
            )
        }
        content()
    }
}

/** Pip in the user's chosen body and color. */
@Composable
fun MyPip(mood: PipMood, size: Dp, look: MascotLook, description: String, modifier: Modifier = Modifier) {
    Pip(mood = mood, size = size, style = look.style, color = look.color, contentDescription = description, modifier = modifier)
}

@Composable
fun Centered(content: @Composable () -> Unit) {
    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { content() }
}
