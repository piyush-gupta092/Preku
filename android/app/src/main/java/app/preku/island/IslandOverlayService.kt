package app.preku.island

import android.annotation.SuppressLint
import android.app.Notification
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.PixelFormat
import android.os.Build
import android.provider.Settings
import android.view.Gravity
import android.view.WindowManager
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.setViewTreeLifecycleOwner
import androidx.savedstate.SavedStateRegistry
import androidx.savedstate.SavedStateRegistryController
import androidx.savedstate.SavedStateRegistryOwner
import androidx.savedstate.setViewTreeSavedStateRegistryOwner
import app.preku.MainActivity
import app.preku.R
import app.preku.data.FocusTimerController
import app.preku.data.IslandSpot
import app.preku.data.SettingsRepository
import app.preku.domain.FocusTimer
import app.preku.domain.formatClock
import app.preku.pip.MascotStyle
import app.preku.pip.Pip
import app.preku.pip.PipMood
import app.preku.platform.Channels
import app.preku.platform.openAppIntent
import app.preku.ui.IslandController
import app.preku.ui.theme.PrekuTheme
import dagger.hilt.android.AndroidEntryPoint
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

/**
 * The floating island (PRD D1): a small Pip pill over other apps, along the top edge. Drag it
 * sideways and it snaps to the left, centre or right; tap it to open Preku. Runs as a
 * foreground service with a quiet notification (D2), which also shows the focus countdown.
 */
@AndroidEntryPoint
class IslandOverlayService : LifecycleService(), SavedStateRegistryOwner {
    @Inject lateinit var timer: FocusTimerController
    @Inject lateinit var settings: SettingsRepository

    private val savedState = SavedStateRegistryController.create(this)
    override val savedStateRegistry: SavedStateRegistry get() = savedState.savedStateRegistry

    private var view: ComposeView? = null
    private val windowManager by lazy { getSystemService(WindowManager::class.java) }
    private val params = WindowManager.LayoutParams(
        WindowManager.LayoutParams.WRAP_CONTENT,
        WindowManager.LayoutParams.WRAP_CONTENT,
        WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
        WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
        PixelFormat.TRANSLUCENT,
    ).apply { gravity = Gravity.TOP or Gravity.START }

    override fun onCreate() {
        savedState.performAttach()
        savedState.performRestore(null)
        super.onCreate()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        if (intent?.action == ACTION_HIDE) {
            lifecycleScope.launch { settings.update { it.copy(islandEnabled = false) } }
            stopSelf()
            return START_NOT_STICKY
        }
        // Without the permission there is nothing to show: stop instead of crashing.
        if (!Settings.canDrawOverlays(this)) {
            stopSelf()
            return START_NOT_STICKY
        }
        startInForeground(notification(null))
        if (view == null) addPill()
        return START_STICKY
    }

    private fun startInForeground(notification: Notification) {
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
        } else {
            0
        }
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, type)
    }

    private fun addPill() {
        val compose = ComposeView(this).apply {
            setViewTreeLifecycleOwner(this@IslandOverlayService)
            setViewTreeSavedStateRegistryOwner(this@IslandOverlayService)
            setContent { PrekuTheme(darkTheme = true) { IslandContent() } }
        }
        params.y = topOffset()
        windowManager.addView(compose, params)
        view = compose
        compose.post { snapTo(settings.settings.value.islandSpot) }

        // Keep the notification's countdown in step with the timer.
        lifecycleScope.launch {
            timer.state.collect { snapshot ->
                val text = if (snapshot.status == FocusTimer.Status.Idle) null else describe(snapshot)
                startInForeground(notification(text))
            }
        }
        lifecycleScope.launch {
            settings.settings.map { it.islandSpot }.distinctUntilChanged().collect(::snapTo)
        }
    }

    @Composable
    private fun IslandContent() {
        val snapshot by timer.state.collectAsState()
        val prefs by settings.settings.collectAsState()
        val running = snapshot.status == FocusTimer.Status.Running
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier
                .background(PrekuTheme.colors.islandBg, RoundedCornerShape(50))
                .border(1.dp, PrekuTheme.colors.islandBorder, RoundedCornerShape(50))
                .pointerInput(Unit) {
                    detectTapGestures(onTap = { openApp() })
                }
                .pointerInput(Unit) {
                    detectHorizontalDragGestures(
                        onDragEnd = { settle() },
                        onHorizontalDrag = { _, dx -> moveBy(dx) },
                    )
                }
                .padding(horizontal = 12.dp, vertical = 6.dp),
        ) {
            Pip(
                mood = if (running) PipMood.Focus else PipMood.Idle,
                size = 24.dp,
                style = MascotStyle.parse(prefs.mascotStyle),
                color = prefs.mascotColor,
                contentDescription = stringResource(R.string.pip_description),
                fps = 15,
                tappable = false,
            )
            Text(
                text = if (snapshot.status == FocusTimer.Status.Idle) {
                    stringResource(R.string.app_name)
                } else {
                    describe(snapshot)
                },
                color = PrekuTheme.colors.islandText,
                style = androidx.compose.material3.MaterialTheme.typography.labelMedium,
            )
        }
    }

    private fun describe(snapshot: FocusTimer.Snapshot): String = getString(
        if (snapshot.status == FocusTimer.Status.Paused) R.string.island_paused else R.string.island_focusing,
        formatClock(snapshot.remainingS),
    )

    private fun moveBy(dx: Float) {
        val v = view ?: return
        val max = (screenWidth() - v.width).coerceAtLeast(0)
        params.x = (params.x + dx.roundToInt()).coerceIn(0, max)
        windowManager.updateViewLayout(v, params)
    }

    /** After a drag: snap to the nearest spot and remember it. */
    private fun settle() {
        val v = view ?: return
        val centre = params.x + v.width / 2f
        val third = screenWidth() / 3f
        val spot = when {
            centre < third -> IslandSpot.Left
            centre > third * 2 -> IslandSpot.Right
            else -> IslandSpot.Center
        }
        snapTo(spot)
        lifecycleScope.launch { settings.update { it.copy(islandSpot = spot) } }
    }

    private fun snapTo(spot: IslandSpot) {
        val v = view ?: return
        val margin = (16 * resources.displayMetrics.density).roundToInt()
        params.x = when (spot) {
            IslandSpot.Left -> margin
            IslandSpot.Center -> (screenWidth() - v.width) / 2
            IslandSpot.Right -> screenWidth() - v.width - margin
        }.coerceAtLeast(0)
        windowManager.updateViewLayout(v, params)
    }

    private fun screenWidth() = resources.displayMetrics.widthPixels

    /** Just below the status bar, so the pill never covers the clock or notch. */
    @SuppressLint("DiscouragedApi", "InternalInsetResource")
    private fun topOffset(): Int {
        val id = resources.getIdentifier("status_bar_height", "dimen", "android")
        val statusBar = if (id > 0) resources.getDimensionPixelSize(id) else 0
        return statusBar + (4 * resources.displayMetrics.density).roundToInt()
    }

    private fun openApp() {
        startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    private fun notification(timerText: String?): Notification {
        val hide = PendingIntent.getService(
            this,
            1,
            Intent(this, IslandOverlayService::class.java).setAction(ACTION_HIDE),
            PendingIntent.FLAG_IMMUTABLE,
        )
        return NotificationCompat.Builder(this, Channels.ISLAND)
            .setSmallIcon(R.drawable.ic_stat_pip)
            .setContentTitle(getString(R.string.island_notification_title))
            .setContentText(timerText ?: getString(R.string.island_notification_text))
            .setContentIntent(openAppIntent(this))
            .addAction(0, getString(R.string.island_hide), hide)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .build()
    }

    override fun onDestroy() {
        view?.let { runCatching { windowManager.removeView(it) } }
        view = null
        super.onDestroy()
    }

    companion object {
        private const val NOTIFICATION_ID = 1
        const val ACTION_HIDE = "app.preku.island.HIDE"
    }
}

class AndroidIslandController @Inject constructor(
    @ApplicationContext private val context: Context,
) : IslandController {
    override fun start() {
        ContextCompat.startForegroundService(context, Intent(context, IslandOverlayService::class.java))
    }

    override fun stop() {
        context.stopService(Intent(context, IslandOverlayService::class.java))
    }
}
