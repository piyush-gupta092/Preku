package app.preku.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

private val LightScheme = lightColorScheme(
    primary = Brand,
    onPrimary = Color.White,
    primaryContainer = BrandHover,
    background = LightSurfaces.bg,
    onBackground = LightSurfaces.text,
    surface = LightSurfaces.surface,
    onSurface = LightSurfaces.text,
    surfaceVariant = LightSurfaces.surface2,
    onSurfaceVariant = LightSurfaces.textMuted,
    outline = LightSurfaces.border,
    error = Danger,
)

private val DarkScheme = darkColorScheme(
    primary = Brand,
    onPrimary = Color.White,
    primaryContainer = BrandHover,
    background = DarkSurfaces.bg,
    onBackground = DarkSurfaces.text,
    surface = DarkSurfaces.surface,
    onSurface = DarkSurfaces.text,
    surfaceVariant = DarkSurfaces.surface2,
    onSurfaceVariant = DarkSurfaces.textMuted,
    outline = DarkSurfaces.border,
    error = Danger,
)

// Design.md §4: radius-sm 8, radius-md 12, radius-lg 18.
private val PrekuShapes = Shapes(
    small = RoundedCornerShape(8.dp),
    medium = RoundedCornerShape(12.dp),
    large = RoundedCornerShape(18.dp),
)

/** Spacing scale (Design.md §4, 4-pt grid). */
object Spacing {
    val s1 = 4.dp
    val s2 = 8.dp
    val s3 = 12.dp
    val s4 = 16.dp
    val s5 = 20.dp
    val s6 = 24.dp
    val s8 = 32.dp
}

@Composable
fun PrekuTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val prekuColors = if (darkTheme) {
        PrekuColors(textMuted = DarkSurfaces.textMuted, border = DarkSurfaces.border)
    } else {
        PrekuColors(textMuted = LightSurfaces.textMuted, border = LightSurfaces.border)
    }
    CompositionLocalProvider(LocalPrekuColors provides prekuColors) {
        MaterialTheme(
            colorScheme = if (darkTheme) DarkScheme else LightScheme,
            typography = PrekuTypography,
            shapes = PrekuShapes,
            content = content,
        )
    }
}

/** Access Preku tokens: `PrekuTheme.colors.islandBg`. */
object PrekuTheme {
    val colors: PrekuColors
        @Composable @ReadOnlyComposable
        get() = LocalPrekuColors.current
}
