package app.preku.bridge

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.BatteryManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import app.preku.R
import app.preku.bridge.protocol.DeviceStatus
import app.preku.platform.Channels
import app.preku.platform.openAppIntent
import dagger.hilt.android.AndroidEntryPoint
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import kotlinx.coroutines.launch

/**
 * Keeps the link to the paired PC alive (foreground service, type connectedDevice), reports
 * battery and network changes, and finds the PC again by mDNS when its address changes.
 */
@AndroidEntryPoint
class BridgeService : LifecycleService() {
    @Inject lateinit var bridge: BridgeManager

    private val connectivity by lazy { getSystemService(ConnectivityManager::class.java) }
    private val nsd by lazy { getSystemService(NsdManager::class.java) }
    private var discovering = false

    private val battery = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) = report(intent)
    }

    private val network = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) {
            bridge.onNetworkAvailable()
            report(null)
        }

        override fun onLost(network: Network) = report(null)
    }

    private val discovery = object : NsdManager.DiscoveryListener {
        override fun onServiceFound(info: NsdServiceInfo) = resolve(info)
        override fun onServiceLost(info: NsdServiceInfo) = Unit
        override fun onDiscoveryStarted(serviceType: String) = Unit
        override fun onDiscoveryStopped(serviceType: String) = Unit
        override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) = Unit
        override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) = Unit
    }

    override fun onCreate() {
        super.onCreate()
        startInForeground(getString(R.string.bridge_looking))
        lifecycleScope.launch {
            bridge.state.collect { s ->
                if (s.pc == null) {
                    // Unpaired (only once the saved pairing has been read).
                    if (bridge.ready.value && s.status == LinkStatus.NotPaired) stopSelf()
                    return@collect
                }
                startInForeground(
                    getString(if (s.status == LinkStatus.Connected) R.string.bridge_connected_to else R.string.bridge_looking_for, s.pc.pcName),
                )
            }
        }
        // A file from the PC arrived: tell the user, tap to open it.
        lifecycleScope.launch {
            val told = mutableSetOf<String>()
            bridge.files.transfers.collect { list ->
                list.filter { it.direction == Direction.Receive && it.state == TransferState.Done && told.add(it.id) }
                    .forEach { notifyReceived(it) }
            }
        }
        ContextCompat.registerReceiver(this, battery, IntentFilter(Intent.ACTION_BATTERY_CHANGED), ContextCompat.RECEIVER_NOT_EXPORTED)
        connectivity?.registerDefaultNetworkCallback(network)
        runCatching {
            nsd?.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, discovery)
            discovering = true
        }
        bridge.start()
    }

    private fun startInForeground(text: String) {
        val notification = NotificationCompat.Builder(this, Channels.BRIDGE)
            .setSmallIcon(R.drawable.ic_stat_pip)
            .setContentTitle(getString(R.string.bridge_notification_title))
            .setContentText(text)
            .setContentIntent(openAppIntent(this))
            .setOngoing(true)
            .setSilent(true)
            .build()
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE else 0
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, type)
    }

    // Checked inside: without the permission (Android 13+) the file still arrives quietly.
    @android.annotation.SuppressLint("MissingPermission")
    private fun notifyReceived(file: TransferUi) {
        if (!app.preku.platform.notificationsAllowed(this)) return
        val open = file.uri?.let { uri ->
            android.app.PendingIntent.getActivity(
                this,
                file.id.hashCode(),
                Intent(Intent.ACTION_VIEW).setDataAndType(uri, contentResolver.getType(uri))
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
                android.app.PendingIntent.FLAG_IMMUTABLE,
            )
        }
        val notification = NotificationCompat.Builder(this, Channels.FILES)
            .setSmallIcon(R.drawable.ic_stat_pip)
            .setContentTitle(getString(R.string.files_received_title))
            .setContentText(getString(R.string.files_received_text, file.name))
            .setContentIntent(open ?: openAppIntent(this))
            .setAutoCancel(true)
            .build()
        androidx.core.app.NotificationManagerCompat.from(this).notify(file.id.hashCode(), notification)
    }

    /** Battery level, charging and network type → `device.status`. */
    private fun report(batteryIntent: Intent?) {
        val intent = batteryIntent
            ?: ContextCompat.registerReceiver(this, null, IntentFilter(Intent.ACTION_BATTERY_CHANGED), ContextCompat.RECEIVER_NOT_EXPORTED)
            ?: return
        val level = intent.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
        val scale = intent.getIntExtra(BatteryManager.EXTRA_SCALE, 100).coerceAtLeast(1)
        val plugged = intent.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0
        val caps = connectivity?.let { it.getNetworkCapabilities(it.activeNetwork) }
        val net = when {
            caps == null -> "none"
            caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> "wifi"
            caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> "ethernet"
            caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> "cellular"
            else -> "none"
        }
        if (level >= 0) bridge.sendStatus(DeviceStatus((level * 100 / scale).coerceIn(0, 100), plugged, net))
    }

    @Suppress("DEPRECATION") // The callback-based resolver only exists from Android 14.
    private fun resolve(found: NsdServiceInfo) {
        runCatching {
            nsd?.resolveService(
                found,
                object : NsdManager.ResolveListener {
                    override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) = Unit
                    override fun onServiceResolved(info: NsdServiceInfo) {
                        val id = info.attributes["id"]?.toString(Charsets.UTF_8) ?: return
                        val host = info.host?.hostAddress ?: return
                        bridge.onPcFound(id, host, info.port)
                    }
                },
            )
        }
    }

    override fun onDestroy() {
        runCatching { unregisterReceiver(battery) }
        runCatching { connectivity?.unregisterNetworkCallback(network) }
        if (discovering) runCatching { nsd?.stopServiceDiscovery(discovery) }
        bridge.stop()
        super.onDestroy()
    }

    companion object {
        private const val NOTIFICATION_ID = 3
        private const val SERVICE_TYPE = "_preku._tcp."
    }
}

/** Starts the bridge service while a PC is paired. */
class BridgeServiceController @Inject constructor(@ApplicationContext private val context: Context) {
    fun ensureRunning() {
        ContextCompat.startForegroundService(context, Intent(context, BridgeService::class.java))
    }
}
