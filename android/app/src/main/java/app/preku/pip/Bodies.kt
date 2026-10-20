package app.preku.pip

import kotlin.math.abs
import kotlin.math.acos
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.sin

// Pip's four bodies (port of `desktop/src/pip/bodies.ts`). Units: ground point (0, 0), up is
// negative y, a full figure is about 220 units tall.

data class Pt(val x: Float, val y: Float)

enum class MascotStyle { Classic, Sprout, Ghost, Chubby;

    companion object {
        fun parse(value: String): MascotStyle = entries.firstOrNull { it.name.equals(value, ignoreCase = true) } ?: Classic
    }
}

internal const val OUTLINE = 4f

internal class Body(
    val shoulder: Pt,
    val hip: Pt,
    val arm: Pair<Float, Float>,
    val armWidth: Float,
    val leg: Pair<Float, Float>,
    val legWidth: Float,
    val handRadius: Float,
    val foot: Pair<Float, Float>,
    /** Eye-line centre and face scale. */
    val face: Pt,
    val faceScale: Float,
    val leaves: Pt,
    val leafScale: Float,
    /** Head centre and radius, for floating effects. */
    val head: Pt,
    val headR: Float,
    /** Head-only crop for small sizes: vertical centre and height. */
    val bustCy: Float,
    val bustH: Float,
    val outline: Boolean = true,
    val wholeInBust: Boolean = false,
    val drawTorso: (Ctx, PipPalette) -> Unit = { _, _ -> },
    val drawHead: (Ctx, PipPalette) -> Unit,
)

private fun outlined(ctx: Ctx, palette: PipPalette, fill: Int) {
    ctx.fill(fill)
    ctx.stroke(palette.outline, OUTLINE)
}

private fun shine(ctx: Ctx, x: Float, y: Float, rx: Float, ry: Float) {
    ctx.save()
    ctx.alpha = 0.45f
    ctx.beginPath()
    ctx.ellipse(x, y, rx, ry, -0.45f)
    ctx.fill(android.graphics.Color.WHITE)
    ctx.restore()
}

/** A round head with a soft shine and a darker chin. */
private fun roundHead(ctx: Ctx, palette: PipPalette, cx: Float, cy: Float, rx: Float, ry: Float) {
    ctx.beginPath()
    ctx.ellipse(cx, cy, rx, ry)
    ctx.fill(palette.body)
    ctx.save()
    ctx.clip()
    ctx.beginPath()
    ctx.ellipse(cx, cy + ry * 0.85f, rx * 0.95f, ry * 0.45f)
    ctx.fill(palette.shade)
    ctx.restore()
    ctx.beginPath()
    ctx.ellipse(cx, cy, rx, ry)
    ctx.stroke(palette.outline, OUTLINE)
    shine(ctx, cx - rx * 0.45f, cy - ry * 0.55f, rx * 0.22f, ry * 0.12f)
}

private val CLASSIC = Body(
    shoulder = Pt(66f, -96f), hip = Pt(30f, -24f), arm = 20f to 17f, armWidth = 17f,
    leg = 9f to 8f, legWidth = 20f, handRadius = 9f, foot = 20f to 11f,
    face = Pt(0f, -110f), faceScale = 2.5f, leaves = Pt(0f, -168f), leafScale = 1.25f,
    head = Pt(0f, -100f), headR = 78f, bustCy = -104f, bustH = 236f,
    outline = false, wholeInBust = true,
    drawHead = { ctx, palette ->
        ctx.beginPath()
        ctx.ellipse(0f, -100f, 90f, 78f)
        ctx.fill(palette.body)
        ctx.save()
        ctx.clip()
        ctx.beginPath()
        ctx.ellipse(0f, -37f, 80f, 40f)
        ctx.fill(palette.shade)
        ctx.restore()
    },
)

private val SPROUT = Body(
    shoulder = Pt(17f, -100f), hip = Pt(9f, -66f), arm = 27f to 25f, armWidth = 9f,
    leg = 30f to 29f, legWidth = 10f, handRadius = 8f, foot = 13f to 8f,
    face = Pt(0f, -144f), faceScale = 1.3f, leaves = Pt(0f, -190f), leafScale = 1f,
    head = Pt(0f, -150f), headR = 45f, bustCy = -166f, bustH = 128f,
    drawTorso = { ctx, palette ->
        ctx.beginPath()
        ctx.ellipse(0f, -84f, 21f, 25f)
        outlined(ctx, palette, palette.body)
    },
    drawHead = { ctx, palette -> roundHead(ctx, palette, 0f, -150f, 47f, 43f) },
)

private fun ghostSheet(ctx: Ctx) {
    ctx.beginPath()
    ctx.moveTo(-48f, -72f)
    ctx.cubicTo(-54f, -152f, 54f, -152f, 48f, -72f)
    for (i in 0 until 6) {
        val x = 48f - i * 16f
        ctx.quadTo(x - 8f, -60f, x - 16f, if (i == 5) -72f else -70f)
    }
    ctx.closePath()
}

private val GHOST = Body(
    shoulder = Pt(44f, -106f), hip = Pt(14f, -58f), arm = 21f to 19f, armWidth = 9f,
    leg = 26f to 25f, legWidth = 10f, handRadius = 8f, foot = 12f to 7f,
    face = Pt(0f, -112f), faceScale = 1.25f, leaves = Pt(0f, -131f), leafScale = 0.9f,
    head = Pt(0f, -110f), headR = 50f, bustCy = -112f, bustH = 116f,
    drawHead = { ctx, palette ->
        ghostSheet(ctx)
        ctx.fill(palette.body)
        ctx.save()
        ctx.clip()
        ctx.beginPath()
        ctx.ellipse(30f, -76f, 34f, 26f, -0.3f)
        ctx.fill(palette.shade)
        ctx.restore()
        ghostSheet(ctx)
        ctx.stroke(palette.outline, OUTLINE)
        ctx.save()
        ctx.alpha = 0.5f
        ctx.beginPath()
        ctx.moveTo(-36f, -82f)
        ctx.cubicTo(-38f, -108f, -30f, -124f, -18f, -132f)
        ctx.stroke(android.graphics.Color.WHITE, 5f)
        ctx.restore()
    },
)

private val CHUBBY = Body(
    shoulder = Pt(30f, -86f), hip = Pt(17f, -40f), arm = 18f to 16f, armWidth = 14f,
    leg = 16f to 15f, legWidth = 16f, handRadius = 9.5f, foot = 15f to 9f,
    face = Pt(0f, -134f), faceScale = 1.35f, leaves = Pt(0f, -181f), leafScale = 1f,
    head = Pt(0f, -140f), headR = 48f, bustCy = -156f, bustH = 128f,
    drawTorso = { ctx, palette ->
        ctx.beginPath()
        ctx.ellipse(0f, -72f, 40f, 36f)
        outlined(ctx, palette, palette.body)
        ctx.beginPath()
        ctx.ellipse(0f, -64f, 22f, 20f)
        ctx.fill(palette.belly)
    },
    drawHead = { ctx, palette -> roundHead(ctx, palette, 0f, -140f, 50f, 44f) },
)

internal fun bodyFor(style: MascotStyle): Body = when (style) {
    MascotStyle.Classic -> CLASSIC
    MascotStyle.Sprout -> SPROUT
    MascotStyle.Ghost -> GHOST
    MascotStyle.Chubby -> CHUBBY
}

data class Limb(val mid: Pt, val end: Pt)

/** Two-bone IK: the elbow/knee for a limb reaching `target`, bending outwards (`side`). */
fun solveLimb(start: Pt, target: Pt, lengths: Pair<Float, Float>, side: Int): Limb {
    val (a, b) = lengths
    val dx = target.x - start.x
    val dy = target.y - start.y
    val reach = hypot(dx, dy)
    val d = reach.coerceIn(abs(a - b) + 0.01f, a + b - 0.01f)
    val base = atan2(dy, dx)
    val offset = acos(((a * a + d * d - b * b) / (2 * a * d)).coerceIn(-1f, 1f))
    val c1 = Pt(start.x + cos(base + offset) * a, start.y + sin(base + offset) * a)
    val c2 = Pt(start.x + cos(base - offset) * a, start.y + sin(base - offset) * a)
    val mid = if (c1.x * side >= c2.x * side) c1 else c2
    val r = if (reach == 0f) 1f else reach
    return Limb(mid, Pt(start.x + dx / r * d, start.y + dy / r * d))
}

data class Arm(val mid: Pt, val end: Pt, val upper: Float)

/** Forward kinematics from one raise angle (0 = hanging, π = up): out first, then up. */
fun armFromAngle(start: Pt, angle: Float, lengths: Pair<Float, Float>, side: Int): Arm {
    val upper = if (angle <= 1.5f) angle * 0.75f else 1.125f + (angle - 1.5f) * 0.6f
    val lower = (angle + if (angle > 1.5f) 0.35f else 0.2f).coerceAtMost(3.1f)
    val mid = Pt(start.x + side * sin(upper) * lengths.first, start.y + cos(upper) * lengths.first)
    val end = Pt(mid.x + side * sin(lower) * lengths.second, mid.y + cos(lower) * lengths.second)
    return Arm(mid, end, upper)
}
