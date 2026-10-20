package app.preku.platform

import android.annotation.SuppressLint
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import app.preku.MainActivity
import app.preku.R
import app.preku.data.FocusNotifier
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject

/** Notification channels (created at startup; Android 8+ needs them before posting). */
object Channels {
    const val ISLAND = "island"
    const val FOCUS = "focus"
    const val BRIDGE = "bridge"
    const val FILES = "files"

    fun create(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannels(
            listOf(
                NotificationChannel(ISLAND, context.getString(R.string.channel_island), NotificationManager.IMPORTANCE_MIN)
                    .apply { description = context.getString(R.string.channel_island_desc) },
                NotificationChannel(FOCUS, context.getString(R.string.channel_focus), NotificationManager.IMPORTANCE_DEFAULT)
                    .apply { description = context.getString(R.string.channel_focus_desc) },
                NotificationChannel(BRIDGE, context.getString(R.string.channel_bridge), NotificationManager.IMPORTANCE_MIN)
                    .apply { description = context.getString(R.string.channel_bridge_desc) },
                NotificationChannel(FILES, context.getString(R.string.channel_files), NotificationManager.IMPORTANCE_DEFAULT)
                    .apply { description = context.getString(R.string.channel_files_desc) },
            ),
        )
    }
}

fun openAppIntent(context: Context): PendingIntent = PendingIntent.getActivity(
    context,
    0,
    Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
)

class AndroidFocusNotifier @Inject constructor(
    @ApplicationContext private val context: Context,
) : FocusNotifier {
    // Checked just above: without the permission (Android 13+) the session just ends quietly.
    @SuppressLint("MissingPermission")
    override fun focusDone(label: String) {
        if (!notificationsAllowed(context)) return
        val notification = NotificationCompat.Builder(context, Channels.FOCUS)
            .setSmallIcon(R.drawable.ic_stat_pip)
            .setContentTitle(context.getString(R.string.focus_done_title))
            .setContentText(context.getString(R.string.focus_done_text, label))
            .setContentIntent(openAppIntent(context))
            .setAutoCancel(true)
            .build()
        NotificationManagerCompat.from(context).notify(FOCUS_DONE_ID, notification)
    }

    private companion object {
        const val FOCUS_DONE_ID = 2
    }
}
