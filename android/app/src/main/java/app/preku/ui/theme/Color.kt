package app.preku.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

// Design tokens — source of truth: /Design.md §2. UI code must use PrekuTheme, never raw colors.

internal val Brand = Color(0xFF7C5CFF)
internal val BrandHover = Color(0xFF6A48F5)

internal val Success = Color(0xFF34C759)
internal val Warning = Color(0xFFFF9F0A)
internal val Danger = Color(0xFFFF453A)
internal val Info = Color(0xFF3DA5FF)

internal object LightSurfaces {
    val bg = Color(0xFFF7F7FA)
    val surface = Color(0xFFFFFFFF)
    val surface2 = Color(0xFFEFEFF4)
    val border = Color(0xFFE2E2EA)
    val text = Color(0xFF14141A)
    val textMuted = Color(0xFF5E5E6B)
}

internal object DarkSurfaces {
    val bg = Color(0xFF0F0F14)
    val surface = Color(0xFF18181F)
    val surface2 = Color(0xFF22222B)
    val border = Color(0xFF2E2E38)
    val text = Color(0xFFF2F2F7)
    val textMuted = Color(0xFFA0A0AD)
}

/** Preku-specific colors that Material's ColorScheme has no slot for. */
@Immutable
data class PrekuColors(
    val islandBg: Color = Color(0xFF0B0B0F),
    val islandBgRaised: Color = Color(0xFF17171D),
    val islandBorder: Color = Color(0x14FFFFFF),
    val islandText: Color = Color(0xFFF5F5F7),
    val islandTextMuted: Color = Color(0xFF9A9AA5),
    val brandSoft: Color = Color(0x297C5CFF),
    val pipBody: Color = Color(0xFFFFB38A),
    val pipCheek: Color = Color(0xFFFF7A7A),
    val pipEye: Color = Color(0xFF1A1423),
    val modProductivity: Color = Color(0xFFFFB020),
    val modBridge: Color = Color(0xFF3DA5FF),
    val modMonitor: Color = Color(0xFF22C7B8),
    val modWellbeing: Color = Color(0xFF5BD16E),
    val success: Color = Success,
    val warning: Color = Warning,
    val danger: Color = Danger,
    val info: Color = Info,
    val textMuted: Color,
    val border: Color,
)

internal val LocalPrekuColors = staticCompositionLocalOf<PrekuColors> {
    error("PrekuColors not provided — wrap content in PrekuTheme")
}
