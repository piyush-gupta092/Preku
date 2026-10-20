package app.preku.platform

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.core.content.ContextCompat
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject

/** What Preku may do right now. Every feature checks this and degrades gracefully when denied. */
data class PermissionState(
    /** "Display over other apps": the floating island. */
    val overlay: Boolean,
    /** Timer and reminder notifications (asked at runtime from Android 13). */
    val notifications: Boolean,
    /** "Usage access": screen time. */
    val usageAccess: Boolean,
    /** Battery optimization off, so the island isn't stopped in the background. */
    val batteryUnrestricted: Boolean,
) {
    val allGranted: Boolean get() = overlay && notifications && usageAccess && batteryUnrestricted

    companion object {
        val NONE = PermissionState(overlay = false, notifications = false, usageAccess = false, batteryUnrestricted = false)
    }
}

interface PermissionChecker {
    fun state(): PermissionState
}

class AndroidPermissions @Inject constructor(
    @ApplicationContext private val context: Context,
    private val screenTime: ScreenTimeSource,
) : PermissionChecker {
    override fun state(): PermissionState = PermissionState(
        overlay = Settings.canDrawOverlays(context),
        notifications = notificationsAllowed(context),
        usageAccess = screenTime.hasAccess(),
        batteryUnrestricted = context.getSystemService(PowerManager::class.java)
            ?.isIgnoringBatteryOptimizations(context.packageName) ?: false,
    )
}

fun notificationsAllowed(context: Context): Boolean =
    Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
        ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
        PackageManager.PERMISSION_GRANTED

/** System settings screens for the special permissions (they can't be asked for in a dialog). */
object PermissionIntents {
    fun overlay(context: Context) =
        Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:${context.packageName}"))

    fun usageAccess() = Intent(Settings.ACTION_USAGE_ACCESS_SETTINGS)

    /**
     * The battery optimization list (not the one-tap request, which Play only allows for a few
     * kinds of apps). The user picks Preku there.
     */
    fun battery() = Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)

    fun appNotifications(context: Context) =
        Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
}
