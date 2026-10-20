package app.preku.pip

import android.graphics.Canvas
import android.graphics.Color
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.sin

// Draws Pip with Canvas shapes, no sprites: a port of `desktop/src/pip/renderer.ts`. The three
// sizes of Pip (24 px pill, cards, big stage) all come from here.

private const val BOX = 270f
private const val GROUND = 0.97f
/** Below this size (dp-independent px) only the head is drawn. */
const val BUST_BELOW = 36f
private const val MOTION = 2.5f
private const val HOP_HEIGHT = 28f

private const val EYE_DX = 13f
private const val EYE_Y = -44f
private const val FACE_Y = -30f
private const val PI_F = PI.toFloat()
private val TAU = Ctx.TAU

private val TEAR = Color.rgb(0x4f, 0xb0, 0xff)
private val HEART = Color.rgb(0xff, 0x4f, 0x6d)
private val TONGUE = Color.rgb(0xff, 0x7a, 0x8a)
private val ACCENT = Color.rgb(0x8b, 0x7b, 0xff)
private val ANGER = Color.rgb(0xff, 0x45, 0x3a)
private val SPARK = Color.rgb(0xff, 0xc9, 0x3c)
private val EXCLAIM = Color.rgb(0xff, 0x9f, 0x0a)
private val THOUGHT = Color.argb(230, 190, 190, 205)
private val CONFETTI = intArrayOf(
    Color.rgb(0xff, 0xb0, 0x20), Color.rgb(0x3d, 0xa5, 0xff), Color.rgb(0x22, 0xc7, 0xb8),
    Color.rgb(0x5b, 0xd1, 0x6e), Color.rgb(0xff, 0x7a, 0x7a), Color.rgb(0xa7, 0x8b, 0xfa),
)

/** Arm raise angle (0 = hanging, π = up) for a mood, exposed for tests. */
fun armAngle(mood: PipMood, time: Float, side: Int): Float = expressionOf(mood).arms(time, side)

private fun facePoint(body: Body, x: Float, y: Float) =
    Pt(body.face.x + x * body.faceScale, body.face.y + (y - EYE_Y) * body.faceScale)

private fun handTarget(body: Body, spec: Expression, side: Int, t: Float): Pt? = when (spec.hands) {
    Hands.Eyes -> facePoint(body, side * EYE_DX, EYE_Y + 2f)
    Hands.Cup -> if (side < 0) {
        null
    } else {
        val k = sip(t)
        val low = facePoint(body, 18f, -4f)
        val mouth = facePoint(body, 6f, -26f)
        Pt(low.x + (mouth.x - low.x) * k, low.y + (mouth.y - low.y) * k)
    }
    Hands.Cheeks -> facePoint(body, side * 22f, -27f)
    Hands.Face -> facePoint(body, side * 17f, -30f)
    Hands.Chin -> if (side > 0) facePoint(body, 7f, -20f) else null
    Hands.Hips -> Pt(side * (body.shoulder.x + 4f), body.shoulder.y + 26f)
    Hands.None -> null
}

private class ArmPose(val side: Int, val start: Pt, val mid: Pt, val end: Pt, val front: Boolean)

/** Draws one frame of Pip into a `size` × `size` px square at the canvas origin. */
fun drawPip(canvas: Canvas, size: Float, pose: PipPose, basePalette: PipPalette, style: MascotStyle) {
    val ctx = Ctx(canvas)
    val body = bodyFor(style)
    val palette = if (body.outline) basePalette else basePalette.copy(outline = basePalette.shade)
    val spec = expressionOf(pose.mood)
    val t = if (pose.still) 0f else pose.time
    val bust = size < BUST_BELOW

    ctx.save()
    if (bust) {
        val k = size / body.bustH
        ctx.translate(size / 2, size / 2)
        ctx.scale(k, k)
        ctx.translate(0f, -body.bustCy)
    } else {
        val k = size / BOX
        ctx.translate(size / 2, size * GROUND)
        ctx.scale(k, k)
    }
    val dance = pose.dance
    if (dance != null) {
        ctx.translate(dance.x * MOTION, -dance.y * MOTION)
        ctx.rotate(dance.tilt)
    } else {
        ctx.translate((spec.shake?.invoke(t) ?: 0f) * MOTION, if (bust) 0f else -pose.hop * HOP_HEIGHT)
        ctx.rotate(spec.tilt?.invoke(t) ?: 0f)
    }
    ctx.scale(pose.breathe * (2 - pose.squish), pose.breathe * pose.squish * spec.stretch)
    val breath = spec.breath?.invoke(t) ?: 1f
    ctx.scale(1 + (breath - 1) * 0.5f, breath)

    if (!bust || body.wholeInBust) {
        val arms = listOf(-1, 1).map { armFor(body, spec, pose, it) }
        drawLegs(ctx, body, pose, palette)
        arms.filter { !it.front }.forEach { drawArm(ctx, body, it, palette) }
        body.drawTorso(ctx, palette)
        drawLeaves(ctx, body, spec, t, palette)
        body.drawHead(ctx, palette)
        drawFace(ctx, body, pose, spec, t, palette)
        arms.filter { it.front }.forEach { arm ->
            if (spec.hands == Hands.Cup && arm.side > 0) {
                noodle(ctx, arm.start, arm.mid, arm.end, body.armWidth, palette)
                drawCup(ctx, body, arm, palette, sip(t))
                drawHand(ctx, body, arm, palette)
            } else {
                drawArm(ctx, body, arm, palette)
            }
        }
        if (!bust) drawFloating(ctx, body, spec, t, palette)
    } else {
        drawLeaves(ctx, body, spec, t, palette)
        body.drawHead(ctx, palette)
        drawFace(ctx, body, pose, spec, t, palette)
    }
    ctx.restore()
}

private fun drawFace(ctx: Ctx, body: Body, pose: PipPose, spec: Expression, t: Float, palette: PipPalette) {
    ctx.save()
    ctx.translate(body.face.x, body.face.y)
    ctx.scale(body.faceScale, body.faceScale)
    ctx.translate(0f, -EYE_Y)
    drawCheeks(ctx, spec, palette)
    drawEyes(ctx, pose, spec, palette)
    drawBrows(ctx, spec, palette)
    drawMouth(ctx, spec, t, palette)
    if (Extra.Tears in spec.extras) drawTears(ctx, t)
    if (Extra.Sweat in spec.extras) drawSweat(ctx, t)
    ctx.restore()
}

private fun armFor(body: Body, spec: Expression, pose: PipPose, side: Int): ArmPose {
    val start = Pt(side * body.shoulder.x, body.shoulder.y)
    val target = if (pose.dance != null) null else handTarget(body, spec, side, if (pose.still) 0f else pose.time)
    if (target != null) {
        val limb = solveLimb(start, target, body.arm, side)
        return ArmPose(side, start, limb.mid, limb.end, front = true)
    }
    val dance = pose.dance
    val angle = when {
        dance != null -> if (side < 0) dance.armL else dance.armR
        else -> spec.arms(if (pose.still) 0f else pose.time, side)
    }
    val arm = armFromAngle(start, angle, body.arm, side)
    return ArmPose(side, start, arm.mid, arm.end, front = arm.upper > 1.1f)
}

private fun noodle(ctx: Ctx, start: Pt, mid: Pt, end: Pt, width: Float, palette: PipPalette) {
    val cx = 2 * mid.x - (start.x + end.x) / 2
    val cy = 2 * mid.y - (start.y + end.y) / 2
    for ((color, w) in listOf(palette.outline to width + OUTLINE * 1.6f, palette.body to width)) {
        ctx.beginPath()
        ctx.moveTo(start.x, start.y)
        ctx.quadTo(cx, cy, end.x, end.y)
        ctx.stroke(color, w)
    }
}

private fun drawArm(ctx: Ctx, body: Body, arm: ArmPose, palette: PipPalette) {
    noodle(ctx, arm.start, arm.mid, arm.end, body.armWidth, palette)
    drawHand(ctx, body, arm, palette)
}

private fun drawCup(ctx: Ctx, body: Body, arm: ArmPose, palette: PipPalette, lifted: Float) {
    val r = body.handRadius
    val w = r * 2.8f
    val h = r * 3.2f
    ctx.save()
    ctx.translate(arm.end.x - r * 1.2f, arm.end.y - r * 0.4f)
    ctx.rotate(-lifted * 0.6f)
    ctx.beginPath()
    ctx.arc(w * 0.55f, 0f, h * 0.22f, -PI_F / 2, PI_F / 2)
    ctx.stroke(palette.outline, OUTLINE * 0.75f)
    ctx.beginPath()
    ctx.moveTo(-w / 2, -h / 2)
    ctx.lineTo(w / 2, -h / 2)
    ctx.lineTo(w * 0.4f, h / 2)
    ctx.lineTo(-w * 0.4f, h / 2)
    ctx.closePath()
    ctx.fill(Color.rgb(0xf7, 0xf5, 0xff))
    ctx.stroke(palette.outline, OUTLINE * 0.75f)
    ctx.beginPath()
    ctx.ellipse(0f, -h / 2 + 3f, w * 0.42f, 3f)
    ctx.fill(Color.rgb(0x7c, 0xc8, 0xff))
    ctx.restore()
}

private fun drawHand(ctx: Ctx, body: Body, arm: ArmPose, palette: PipPalette) {
    val r = body.handRadius
    val dir = atan2(arm.end.y - arm.mid.y, arm.end.x - arm.mid.x)
    val thumb = dir - arm.side * 1.2f
    for ((x, y, radius) in listOf(
        Triple(arm.end.x + cos(thumb) * r * 0.85f, arm.end.y + sin(thumb) * r * 0.85f, r * 0.45f),
        Triple(arm.end.x, arm.end.y, r),
    )) {
        ctx.beginPath()
        ctx.arc(x, y, radius, 0f, TAU)
        ctx.fill(palette.body)
        ctx.stroke(palette.outline, OUTLINE * 0.85f)
    }
}

private fun drawLegs(ctx: Ctx, body: Body, pose: PipPose, palette: PipPalette) {
    val (rx, ry) = body.foot
    for (side in listOf(-1, 1)) {
        val step = pose.dance?.let { if (side < 0) it.footL else it.footR }
        val hip = Pt(side * body.hip.x, body.hip.y)
        val target = Pt(
            side * (body.hip.x + 3f + (step?.dx ?: 0f) * MOTION),
            -ry * 1.4f - (step?.lift ?: 0f) * MOTION - pose.hop * 10f,
        )
        val leg = solveLimb(hip, target, body.leg, side)
        noodle(ctx, hip, leg.mid, leg.end, body.legWidth, palette)
        val x = leg.end.x + side * rx * 0.3f
        val y = leg.end.y + ry * 0.4f
        ctx.beginPath()
        ctx.ellipse(x, y, rx, ry)
        ctx.fill(palette.body)
        ctx.save()
        ctx.clip()
        ctx.beginPath()
        ctx.ellipse(x, y + ry * 0.7f, rx, ry * 0.5f)
        ctx.fill(palette.shade)
        ctx.restore()
        ctx.beginPath()
        ctx.ellipse(x, y, rx, ry)
        ctx.stroke(palette.outline, OUTLINE * 0.85f)
    }
}

private fun drawLeaves(ctx: Ctx, body: Body, spec: Expression, t: Float, palette: PipPalette) {
    val s = body.leafScale
    for (side in listOf(-1, 1)) {
        val wiggle = if (spec.leafWiggle) sin(t / 90f + side) * 0.12f else 0f
        ctx.save()
        ctx.translate(body.leaves.x, body.leaves.y)
        ctx.rotate(side * spec.leaves * 0.03f + wiggle)
        ctx.scale(s, s)
        ctx.beginPath()
        ctx.moveTo(0f, 0f)
        ctx.cubicTo(side * 6f, -30f, side * 34f, -34f, side * 40f, -22f)
        ctx.cubicTo(side * 34f, -6f, side * 14f, 2f, 0f, 0f)
        ctx.closePath()
        ctx.fill(palette.leaf)
        ctx.stroke(palette.outline, OUTLINE * 0.85f)
        ctx.beginPath()
        ctx.moveTo(side * 4f, -4f)
        ctx.quadTo(side * 20f, -18f, side * 34f, -21f)
        ctx.stroke(palette.leafVein, 2.5f)
        ctx.restore()
    }
}

private fun drawCheeks(ctx: Ctx, spec: Expression, palette: PipPalette) {
    val blush = spec.blush
    val r = if (spec.puff) 7.5f else if (blush > 0.75f) 6.5f else 5.5f
    ctx.save()
    for (side in listOf(-1, 1)) {
        ctx.alpha = blush
        ctx.beginPath()
        ctx.arc(side * 24f, FACE_Y, r, 0f, TAU)
        ctx.fill(palette.cheek)
        if (blush > 0.75f && !spec.puff) {
            ctx.alpha = 0.45f
            for (dx in listOf(-3f, 0f, 3f)) {
                ctx.beginPath()
                ctx.moveTo(side * 24f + dx - 1f, FACE_Y + 2f)
                ctx.lineTo(side * 24f + dx + 1f, FACE_Y - 2f)
                ctx.stroke(palette.eye, 1.2f)
            }
        }
    }
    ctx.restore()
}

private fun drawEyes(ctx: Ctx, pose: PipPose, spec: Expression, palette: PipPalette) {
    val look = spec.look ?: Look(pose.lookX, pose.lookY)
    val lookX = look.x * 3f
    val lookY = look.y * 2.5f
    for (side in listOf(-1, 1)) {
        val x = side * EYE_DX + lookX
        val y = EYE_Y + lookY
        val kind = if (spec.eyes == EyeKind.Wink) (if (side > 0) EyeKind.Smile else EyeKind.Bright) else spec.eyes
        when (kind) {
            EyeKind.Smile -> {
                ctx.beginPath()
                ctx.arc(x, y + 3f, 5f, PI_F * 1.15f, PI_F * 1.85f)
                ctx.stroke(palette.eye, 3f)
                continue
            }
            EyeKind.Closed -> {
                ctx.beginPath()
                ctx.moveTo(x - 5f, y + 1f)
                ctx.quadTo(x, y + 4f, x + 5f, y + 1f)
                ctx.stroke(palette.eye, 3f)
                continue
            }
            EyeKind.Laugh -> {
                ctx.beginPath()
                ctx.moveTo(x - side * 4f, y - 4f)
                ctx.lineTo(x + side * 3f, y)
                ctx.lineTo(x - side * 4f, y + 4f)
                ctx.stroke(palette.eye, 3f)
                continue
            }
            EyeKind.Spiral -> {
                drawSpiral(ctx, x, y, pose.time * side, palette.eye)
                continue
            }
            EyeKind.Heart -> {
                drawHeart(ctx, x, y + 1f, 12f, HEART)
                continue
            }
            EyeKind.Ring -> {
                val r = 6f * spec.eyeScale
                ctx.beginPath()
                ctx.arc(x, y, r, 0f, TAU)
                ctx.stroke(palette.eye, 2f)
                ctx.beginPath()
                ctx.arc(x, y, 2f, 0f, TAU)
                ctx.fill(palette.eye)
                continue
            }
            else -> Unit
        }
        val big = spec.eyeScale
        val lid = if (kind == EyeKind.Flat) 0.45f else spec.eyeOpen
        val open = pose.eyeOpen * lid
        val rx = 5.5f * big
        val ry = 8f * big * max(open, 0.08f)
        ctx.beginPath()
        ctx.ellipse(x, y, rx, ry)
        ctx.fill(palette.eye)
        if (kind == EyeKind.Flat) {
            ctx.beginPath()
            ctx.moveTo(x - rx - 1.5f, y - ry)
            ctx.lineTo(x + rx + 1.5f, y - ry)
            ctx.stroke(palette.eye, 2.5f)
        }
        if (open > 0.5f) {
            val shine = when (kind) {
                EyeKind.Watery -> 2.6f
                EyeKind.Bright -> 2.2f
                else -> 1.7f
            }
            ctx.beginPath()
            ctx.arc(x + 1.8f, y - ry * 0.4f, shine, 0f, TAU)
            ctx.fill(palette.highlight)
            if (kind == EyeKind.Bright || kind == EyeKind.Watery) {
                ctx.beginPath()
                ctx.arc(x - 1.6f, y + ry * 0.35f, shine * 0.5f, 0f, TAU)
                ctx.fill(palette.highlight)
            }
            if (kind == EyeKind.Watery) {
                ctx.beginPath()
                ctx.ellipse(x, y + ry * 0.6f, rx * 0.85f, ry * 0.32f)
                ctx.fill(Color.argb(191, 120, 190, 255))
            }
        }
    }
}

private fun drawBrows(ctx: Ctx, spec: Expression, palette: PipPalette) {
    if (spec.brows == BrowKind.None) return
    val top = EYE_Y - 8f * spec.eyeScale
    for (side in listOf(-1, 1)) {
        val inner = side * 6f
        val outer = side * 20f
        ctx.beginPath()
        when (spec.brows) {
            BrowKind.Angry -> {
                ctx.moveTo(inner, top - 2f)
                ctx.lineTo(outer, top - 8f)
            }
            BrowKind.Worried -> {
                ctx.moveTo(inner, top - 9f)
                ctx.lineTo(outer, top - 4f)
            }
            BrowKind.Raised -> ctx.arc(side * EYE_DX, top - 2f, 6f, PI_F * 1.2f, PI_F * 1.8f)
            BrowKind.OneUp -> if (side > 0) {
                ctx.arc(side * EYE_DX, top - 3f, 6f, PI_F * 1.2f, PI_F * 1.8f)
            } else {
                ctx.moveTo(inner, top - 4f)
                ctx.lineTo(outer, top - 4f)
            }
            BrowKind.None -> Unit
        }
        ctx.stroke(palette.eye, 2.6f)
    }
}

private fun drawSpiral(ctx: Ctx, x: Float, y: Float, time: Float, color: Int) {
    val turn = time / 120f
    ctx.beginPath()
    for (i in 0..24) {
        val a = turn + i / 24f * PI_F * 3f
        val r = i / 24f * 6f
        val px = x + cos(a) * r
        val py = y + sin(a) * r
        if (i == 0) ctx.moveTo(px, py) else ctx.lineTo(px, py)
    }
    ctx.stroke(color, 3f)
}

private fun drawHeart(ctx: Ctx, x: Float, y: Float, size: Float, color: Int) {
    val s = size / 2
    ctx.beginPath()
    ctx.moveTo(x, y + s * 0.9f)
    ctx.cubicTo(x - s * 1.6f, y - s * 0.1f, x - s * 0.7f, y - s * 1.3f, x, y - s * 0.45f)
    ctx.cubicTo(x + s * 0.7f, y - s * 1.3f, x + s * 1.6f, y - s * 0.1f, x, y + s * 0.9f)
    ctx.fill(color)
}

private fun drawMouth(ctx: Ctx, spec: Expression, t: Float, palette: PipPalette) {
    val y = FACE_Y
    val ink = palette.eye
    ctx.beginPath()
    when (spec.mouth) {
        MouthKind.None -> Unit
        MouthKind.Small -> {
            ctx.arc(0f, y - 2f, 3f, PI_F * 0.15f, PI_F * 0.85f)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Smile -> {
            ctx.arc(0f, y - 3f, 6f, PI_F * 0.15f, PI_F * 0.85f)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Smirk -> {
            ctx.moveTo(-4f, y)
            ctx.quadTo(1f, y + 3f, 6f, y - 2f)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Shy -> {
            ctx.arc(-3f, y - 1f, 2.5f, PI_F * 0.15f, PI_F * 0.85f)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Grin, MouthKind.Laugh -> {
            val r = if (spec.mouth == MouthKind.Laugh) 6f + abs(sin(t / 90f)) * 1.2f else 6f
            ctx.arc(0f, y - 2f, r, 0f, PI_F)
            ctx.closePath()
            ctx.fill(ink)
            ctx.save()
            ctx.clip()
            ctx.beginPath()
            ctx.ellipse(0f, y - 2f + r, r * 0.6f, r * 0.45f)
            ctx.fill(TONGUE)
            ctx.restore()
        }
        MouthKind.Sing -> {
            ctx.ellipse(0f, y - 1f, 3.5f, 3f + abs(sin(t / 250f)) * 1.5f)
            ctx.fill(ink)
        }
        MouthKind.O -> {
            ctx.ellipse(0f, y, 2.8f, 3.4f)
            ctx.fill(ink)
        }
        MouthKind.BigO -> {
            ctx.ellipse(0f, y + 1f, 3.8f, 5.5f)
            ctx.fill(ink)
        }
        MouthKind.Frown -> {
            ctx.arc(0f, y + 3f, 4f, PI_F * 1.2f, PI_F * 1.8f)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Pout -> {
            ctx.arc(0f, y + 2f, 3f, PI_F * 1.2f, PI_F * 1.8f)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Tremble -> {
            for (i in 0..10) {
                val px = -5f + i
                val py = y + 2f - (1f - (px / 5f) * (px / 5f)) * 2.5f + sin(px * 2f + t / 40f) * 0.6f
                if (i == 0) ctx.moveTo(px, py) else ctx.lineTo(px, py)
            }
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Wavy -> {
            ctx.moveTo(-4f, y)
            ctx.quadTo(-2f, y - 2f, 0f, y)
            ctx.quadTo(2f, y + 2f, 4f, y)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Flat -> {
            ctx.moveTo(-4f, y)
            ctx.lineTo(4f, y)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Awkward -> {
            ctx.moveTo(-5f, y - 1f)
            ctx.lineTo(-2.5f, y + 1f)
            ctx.lineTo(0f, y - 1f)
            ctx.lineTo(2.5f, y + 1f)
            ctx.lineTo(5f, y - 1f)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Tongue -> {
            ctx.ellipse(2.5f, y + 4f, 3.4f, 4.2f)
            ctx.fill(TONGUE)
            ctx.stroke(ink, 1.4f)
            ctx.beginPath()
            ctx.moveTo(2.5f, y + 2f)
            ctx.lineTo(2.5f, y + 5f)
            ctx.stroke(ink, 1.4f)
            ctx.beginPath()
            ctx.arc(0f, y - 3f, 6f, PI_F * 0.12f, PI_F * 0.88f)
            ctx.stroke(ink, 2.5f)
        }
        MouthKind.Yawn -> {
            val phase = (t % 4000f) / 4000f
            val open = if (phase < 0.4f) sin(phase / 0.4f * PI_F) else 0f
            if (open > 0.05f) {
                ctx.ellipse(0f, y + 1f, 2.5f + open * 1.5f, 1f + open * 4.5f)
                ctx.fill(ink)
            }
        }
    }
}

private fun drawTears(ctx: Ctx, t: Float) {
    ctx.save()
    for (side in listOf(-1, 1)) {
        val p = (((t / 900f + if (side > 0) 0.5f else 0f) % 1f) + 1f) % 1f
        ctx.alpha = 1f - p * 0.7f
        drawDrop(ctx, side * (EYE_DX + 4f), EYE_Y + 8f + p * 16f, 2.4f, TEAR)
    }
    ctx.restore()
}

private fun drawSweat(ctx: Ctx, t: Float) {
    ctx.save()
    ctx.alpha = 0.9f
    val dy = (t / 60f) % 5f
    drawDrop(ctx, 30f, -62f + dy, 3.4f, TEAR)
    ctx.beginPath()
    ctx.arc(29f, -61f + dy, 0.9f, 0f, TAU)
    ctx.fill(Color.WHITE)
    ctx.restore()
}

private fun drawDrop(ctx: Ctx, x: Float, y: Float, r: Float, color: Int) {
    ctx.beginPath()
    ctx.moveTo(x, y - r * 2.2f)
    ctx.quadTo(x + r * 1.1f, y - r * 0.4f, x + r, y)
    ctx.arc(x, y, r, 0f, PI_F)
    ctx.quadTo(x - r * 1.1f, y - r * 0.4f, x, y - r * 2.2f)
    ctx.fill(color)
}

/** Effects floating around the head, laid out in the original 100-unit space. */
private fun drawFloating(ctx: Ctx, body: Body, spec: Expression, t: Float, palette: PipPalette) {
    if (spec.extras.isEmpty()) return
    ctx.save()
    val k = body.headR / 31f
    ctx.translate(body.head.x, body.head.y)
    ctx.scale(k, k)
    ctx.translate(-50f, -62.6f)
    for (extra in spec.extras) {
        ctx.save()
        when (extra) {
            Extra.Notes -> for ((offset, x) in listOf(0f to 84f, 0.5f to 14f)) {
                val p = (t / 1600f + offset) % 1f
                ctx.alpha = sin(p * PI_F) * 0.9f
                ctx.text(if (offset == 0f) "♪" else "♫", x + sin(p * 6f) * 3f, 44f - p * 34f, 16f, palette.leaf)
            }
            Extra.Hearts -> for ((offset, x) in listOf(0f to 84f, 0.5f to 16f)) {
                val p = (t / 1800f + offset) % 1f
                ctx.alpha = sin(p * PI_F)
                drawHeart(ctx, x + sin(p * 6f) * 3f, 40f - p * 32f, 8f + p * 3f, HEART)
            }
            Extra.Zzz -> for (i in 0 until 3) {
                val p = (t / 2400f + i / 3f) % 1f
                ctx.alpha = sin(p * PI_F)
                ctx.text("z", 74f + p * 16f, 34f - p * 26f, 8f + p * 8f, ACCENT)
            }
            Extra.Question -> ctx.text("?", 86f, 26f + sin(t / 300f) * 2f, 20f, ACCENT)
            Extra.Exclaim -> {
                val pop = 1f + abs(sin(t / 250f)) * 0.15f
                ctx.translate(86f, 24f)
                ctx.scale(pop, pop)
                ctx.text("!", 0f, 0f, 20f, EXCLAIM)
            }
            Extra.Anger -> {
                val pulse = 1f + abs(sin(t / 200f)) * 0.2f
                ctx.translate(84f, 18f)
                ctx.scale(pulse, pulse)
                repeat(4) {
                    ctx.rotate(PI_F / 2)
                    ctx.beginPath()
                    ctx.arc(5f, 5f, 3.5f, PI_F, PI_F * 1.5f)
                    ctx.stroke(ANGER, 2.2f)
                }
            }
            Extra.Sparkle -> for ((x, y, phase) in listOf(Triple(14f, 22f, 0f), Triple(87f, 16f, 1.3f), Triple(89f, 48f, 2.6f))) {
                drawStar(ctx, x, y, 2f + abs(sin(t / 300f + phase)) * 3f, SPARK)
            }
            Extra.Thought -> for ((i, c) in listOf(Triple(70f, 30f, 1.6f), Triple(78f, 21f, 2.6f), Triple(88f, 10f, 4f)).withIndex()) {
                ctx.alpha = if ((t / 400f) % 4f > i) 1f else 0.25f
                ctx.beginPath()
                ctx.arc(c.first, c.second, c.third, 0f, TAU)
                ctx.fill(THOUGHT)
            }
            Extra.Confetti -> for (i in 0 until 8) {
                val x = 8f + i * 12f + sin(t / 300f + i) * 3f
                val y = ((t / 18f + i * 37f) % 60f) - 6f
                ctx.save()
                ctx.translate(x, y)
                ctx.rotate(t / 200f + i)
                ctx.alpha = 1f - y / 70f
                ctx.beginPath()
                ctx.moveTo(-2f, -1f)
                ctx.lineTo(2f, -1f)
                ctx.lineTo(2f, 1.2f)
                ctx.lineTo(-2f, 1.2f)
                ctx.closePath()
                ctx.fill(CONFETTI[i % CONFETTI.size])
                ctx.restore()
            }
            Extra.Tears, Extra.Sweat -> Unit
        }
        ctx.restore()
    }
    ctx.restore()
}

private fun drawStar(ctx: Ctx, x: Float, y: Float, s: Float, color: Int) {
    ctx.beginPath()
    ctx.moveTo(x, y - s)
    ctx.quadTo(x, y, x + s, y)
    ctx.quadTo(x, y, x, y + s)
    ctx.quadTo(x, y, x - s, y)
    ctx.quadTo(x, y, x, y - s)
    ctx.fill(color)
}
