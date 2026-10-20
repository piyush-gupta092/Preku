package app.preku.pip

import android.provider.Settings
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp

/**
 * Pip, animated (same drawing as the desktop). Tapping squishes Pip; five quick taps make it
 * dizzy. With "Remove animations" on, Pip holds still (and doesn't dance).
 */
@Composable
fun Pip(
    mood: PipMood,
    size: Dp,
    style: MascotStyle,
    color: String,
    contentDescription: String,
    modifier: Modifier = Modifier,
    /** Lower when Pip is small or in the background, to save battery. */
    fps: Int = 30,
    tappable: Boolean = true,
) {
    val context = LocalContext.current
    val reducedMotion = remember {
        Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    }
    val animator = remember { PipAnimator(System.currentTimeMillis()) }
    val palette = remember(color) { paletteFor(color) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }

    LaunchedEffect(fps, reducedMotion) {
        val frameMs = 1000L / fps.coerceIn(1, 60)
        var last = 0L
        while (true) {
            withFrameMillis { frame ->
                if (frame - last >= frameMs) {
                    last = frame
                    now = System.currentTimeMillis()
                }
            }
        }
    }

    val tap = if (tappable) {
        Modifier.clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {
            animator.poke(System.currentTimeMillis())
        }
    } else {
        Modifier
    }

    Canvas(
        modifier
            .size(size)
            .then(tap)
            .semantics { this.contentDescription = contentDescription },
    ) {
        val pose = animator.pose(now, mood, reducedMotion)
        drawIntoCanvas { canvas ->
            // Draw in dp, so size thresholds (head-only below 36) match the desktop's CSS px.
            val native = canvas.nativeCanvas
            native.save()
            native.scale(density, density)
            drawPip(native, this.size.minDimension / density, pose, palette, style)
            native.restore()
        }
    }
}
