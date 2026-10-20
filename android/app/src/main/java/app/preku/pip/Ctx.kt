package app.preku.pip

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Typeface
import kotlin.math.PI
import kotlin.math.roundToInt

/**
 * A thin Canvas 2D-style wrapper over Android's [Canvas], so Pip's drawing code reads (almost)
 * line for line like the desktop renderer (`desktop/src/pip/renderer.ts`). Angles are radians,
 * y points down, arcs sweep clockwise: the same conventions as the browser canvas.
 */
internal class Ctx(val canvas: Canvas) {
    private val path = Path()
    private val rect = RectF()
    private val fillPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }
    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
    }
    private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        textAlign = Paint.Align.CENTER
        typeface = Typeface.DEFAULT_BOLD
    }
    private val alphas = ArrayDeque<Float>()

    /** Like `globalAlpha`: restored by [restore]. */
    var alpha = 1f

    fun save() {
        canvas.save()
        alphas.addLast(alpha)
    }

    fun restore() {
        canvas.restore()
        alpha = alphas.removeLastOrNull() ?: 1f
    }

    fun translate(x: Float, y: Float) = canvas.translate(x, y)
    fun scale(x: Float, y: Float) = canvas.scale(x, y)
    fun rotate(radians: Float) = canvas.rotate(Math.toDegrees(radians.toDouble()).toFloat())

    fun beginPath() = path.reset()
    fun moveTo(x: Float, y: Float) = path.moveTo(x, y)
    fun lineTo(x: Float, y: Float) = path.lineTo(x, y)
    fun quadTo(cx: Float, cy: Float, x: Float, y: Float) = path.quadTo(cx, cy, x, y)
    fun cubicTo(c1x: Float, c1y: Float, c2x: Float, c2y: Float, x: Float, y: Float) =
        path.cubicTo(c1x, c1y, c2x, c2y, x, y)
    fun closePath() = path.close()

    /** `arc(x, y, r, start, end)`; joins the current point with a line, like the browser. */
    fun arc(x: Float, y: Float, r: Float, start: Float, end: Float) {
        rect.set(x - r, y - r, x + r, y + r)
        val sweep = Math.toDegrees((end - start).toDouble()).toFloat()
        if (sweep >= 359.9f) {
            path.addOval(rect, Path.Direction.CW)
        } else {
            path.arcTo(rect, Math.toDegrees(start.toDouble()).toFloat(), sweep, path.isEmpty)
        }
    }

    /** A full ellipse (`ellipse(x, y, rx, ry, rotation, 0, 2π)`). */
    fun ellipse(x: Float, y: Float, rx: Float, ry: Float, rotation: Float = 0f) {
        if (rotation == 0f) {
            rect.set(x - rx, y - ry, x + rx, y + ry)
            path.addOval(rect, Path.Direction.CW)
        } else {
            val oval = Path().apply { addOval(RectF(-rx, -ry, rx, ry), Path.Direction.CW) }
            val m = android.graphics.Matrix().apply {
                setRotate(Math.toDegrees(rotation.toDouble()).toFloat())
                postTranslate(x, y)
            }
            oval.transform(m)
            path.addPath(oval)
        }
    }

    /** Lower half of an ellipse (`ellipse(..., 0, π)`), used for soft shades. */
    fun halfEllipse(x: Float, y: Float, rx: Float, ry: Float) {
        rect.set(x - rx, y - ry, x + rx, y + ry)
        path.arcTo(rect, 0f, 180f, true)
        path.close()
    }

    fun fill(color: Int) {
        fillPaint.color = color
        fillPaint.alpha = (android.graphics.Color.alpha(color) * alpha).roundToInt().coerceIn(0, 255)
        canvas.drawPath(path, fillPaint)
    }

    fun stroke(color: Int, width: Float) {
        strokePaint.color = color
        strokePaint.strokeWidth = width
        strokePaint.alpha = (android.graphics.Color.alpha(color) * alpha).roundToInt().coerceIn(0, 255)
        canvas.drawPath(path, strokePaint)
    }

    fun clip() {
        canvas.clipPath(path)
    }

    fun text(text: String, x: Float, y: Float, size: Float, color: Int) {
        textPaint.textSize = size
        textPaint.color = color
        textPaint.alpha = (android.graphics.Color.alpha(color) * alpha).roundToInt().coerceIn(0, 255)
        canvas.drawText(text, x, y, textPaint)
    }

    companion object {
        const val TAU = (PI * 2).toFloat()
        const val PIF = PI.toFloat()
    }
}
