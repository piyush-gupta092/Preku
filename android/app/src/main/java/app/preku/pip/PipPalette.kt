package app.preku.pip

import android.graphics.Color
import kotlin.math.roundToInt

/** Pip's colors, all derived from the one body color the user picks (port of `palette.ts`). */
data class PipPalette(
    val body: Int,
    val shade: Int,
    val belly: Int,
    val outline: Int,
    val cheek: Int,
    val eye: Int,
    val leaf: Int,
    val leafVein: Int,
    val highlight: Int,
)

const val DEFAULT_MASCOT_COLOR = "#ffb38a"

/** Presets offered in Settings (same as the desktop). */
val MASCOT_COLORS = listOf(
    "peach" to "#ffb38a",
    "lemon" to "#ffd86b",
    "mint" to "#9be3b4",
    "sky" to "#9cc8ff",
    "lavender" to "#c6b2ff",
    "bubblegum" to "#ffa6c9",
    "cocoa" to "#c99a78",
    "snow" to "#f4f1ea",
)

private fun parse(hex: String): IntArray {
    val clean = hex.trim().removePrefix("#")
    val value = if (clean.length == 6 && clean.all { it.isDigit() || it.lowercaseChar() in 'a'..'f' }) {
        clean.toInt(16)
    } else {
        DEFAULT_MASCOT_COLOR.removePrefix("#").toInt(16)
    }
    return intArrayOf((value shr 16) and 255, (value shr 8) and 255, value and 255)
}

private fun mix(a: IntArray, b: IntArray, amount: Float): IntArray =
    IntArray(3) { i -> (a[i] + (b[i] - a[i]) * amount).roundToInt() }

private fun rgb(c: IntArray, alpha: Float = 1f): Int =
    Color.argb((alpha * 255).roundToInt(), c[0], c[1], c[2])

fun paletteFor(color: String): PipPalette {
    val body = parse(color)
    val ink = intArrayOf(42, 22, 12)
    return PipPalette(
        body = rgb(body),
        shade = rgb(mix(body, ink, 0.35f), 0.35f),
        belly = rgb(mix(body, intArrayOf(255, 255, 255), 0.45f)),
        outline = rgb(mix(body, ink, 0.82f)),
        cheek = rgb(mix(intArrayOf(255, 122, 122), body, 0.15f)),
        eye = Color.rgb(0x1a, 0x14, 0x23),
        leaf = Color.rgb(0x6c, 0xc9, 0x5c),
        leafVein = Color.rgb(0x4a, 0xa5, 0x3c),
        highlight = Color.WHITE,
    )
}
