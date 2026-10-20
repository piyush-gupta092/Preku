package app.preku.platform

import android.app.AppOpsManager
import android.app.usage.UsageStatsManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Process
import app.preku.domain.Clock
import app.preku.domain.startOfDay
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

data class AppUsage(val label: String, val seconds: Long)

data class ScreenTimeToday(val totalS: Long, val apps: List<AppUsage>)

/** Today's screen time (PRD W4). App names only, never what's on screen. */
interface ScreenTimeSource {
    /** The "Usage access" special permission, granted in system settings. */
    fun hasAccess(): Boolean
    suspend fun today(): ScreenTimeToday?
}

class UsageStatsScreenTime @Inject constructor(
    @ApplicationContext private val context: Context,
    private val clock: Clock,
) : ScreenTimeSource {

    override fun hasAccess(): Boolean {
        val ops = context.getSystemService(AppOpsManager::class.java) ?: return false
        return checkUsageOp(ops) == AppOpsManager.MODE_ALLOWED
    }

    @Suppress("DEPRECATION") // checkOpNoThrow is the only option before Android 10.
    private fun checkUsageOp(ops: AppOpsManager): Int {
        val uid = Process.myUid()
        val pkg = context.packageName
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            ops.unsafeCheckOpNoThrow(AppOpsManager.OPSTR_GET_USAGE_STATS, uid, pkg)
        } else {
            ops.checkOpNoThrow(AppOpsManager.OPSTR_GET_USAGE_STATS, uid, pkg)
        }
    }

    override suspend fun today(): ScreenTimeToday? = withContext(Dispatchers.IO) {
        if (!hasAccess()) return@withContext null
        val manager = context.getSystemService(UsageStatsManager::class.java) ?: return@withContext null
        val stats = manager.queryAndAggregateUsageStats(startOfDay(clock.today()), clock.now())
        val apps = stats.values
            .filter { it.totalTimeInForeground >= MIN_MS && it.packageName != context.packageName }
            .map { AppUsage(labelOf(it.packageName), it.totalTimeInForeground / 1000) }
            .groupBy { it.label }
            .map { (label, rows) -> AppUsage(label, rows.sumOf { it.seconds }) }
            .sortedByDescending { it.seconds }
        ScreenTimeToday(apps.sumOf { it.seconds }, apps.take(TOP_APPS))
    }

    private fun labelOf(packageName: String): String = runCatching {
        val pm = context.packageManager
        pm.getApplicationLabel(applicationInfo(pm, packageName)).toString()
    }.getOrDefault(packageName.substringAfterLast('.'))

    @Suppress("DEPRECATION") // The flags overload only exists from Android 13.
    private fun applicationInfo(pm: PackageManager, packageName: String) =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            pm.getApplicationInfo(packageName, PackageManager.ApplicationInfoFlags.of(0))
        } else {
            pm.getApplicationInfo(packageName, 0)
        }

    private companion object {
        /** Ignore blips under a minute (launchers, quick switches). */
        const val MIN_MS = 60_000L
        const val TOP_APPS = 6
    }
}
