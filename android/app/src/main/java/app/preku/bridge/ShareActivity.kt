package app.preku.bridge

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.lifecycle.lifecycleScope
import app.preku.R
import dagger.hilt.android.AndroidEntryPoint
import java.io.File
import javax.inject.Inject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * "Share → Preku" from any app: sends the shared files to the paired PC. Android may take back
 * the right to read a shared file once this screen closes, so the files are first copied into
 * Preku's cache (while the screen is still open), then sent from there. Progress shows in
 * Settings → Your PC.
 */
@AndroidEntryPoint
class ShareActivity : ComponentActivity() {
    @Inject lateinit var bridge: BridgeManager

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val uris = sharedUris(intent)
        val state = bridge.state.value
        val pcName = state.pc?.pcName.orEmpty()
        val problem = when {
            uris.isEmpty() -> R.string.share_nothing
            state.pc == null -> R.string.share_not_paired
            state.status != LinkStatus.Connected -> R.string.share_not_connected
            else -> null
        }
        if (problem != null) {
            Toast.makeText(this, getString(problem, pcName), Toast.LENGTH_LONG).show()
            finish()
            return
        }
        Toast.makeText(this, getString(R.string.share_sending, pcName), Toast.LENGTH_SHORT).show()
        lifecycleScope.launch {
            val copies = withContext(Dispatchers.IO) { copyToCache(uris) }
            bridge.files.sendCopies(copies)
            finish()
        }
    }

    private fun copyToCache(uris: List<Uri>): List<File> {
        val dir = File(cacheDir, "share").apply {
            // Old copies from earlier shares are no longer needed.
            listFiles()?.forEach { it.delete() }
            mkdirs()
        }
        return uris.mapIndexedNotNull { index, uri ->
            runCatching {
                val name = sanitizeName(displayName(uri) ?: "file-${index + 1}")
                val target = File(dir, "$index-$name").let { File(dir, name).takeIf { f -> !f.exists() } ?: it }
                contentResolver.openInputStream(uri)!!.use { input -> target.outputStream().use { input.copyTo(it) } }
                target
            }.getOrNull()
        }
    }

    private fun displayName(uri: Uri): String? =
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        }

    @Suppress("DEPRECATION") // The typed getParcelableExtra overloads only exist from Android 13.
    private fun sharedUris(intent: Intent): List<Uri> = when (intent.action) {
        Intent.ACTION_SEND -> listOfNotNull(
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            } else {
                intent.getParcelableExtra(Intent.EXTRA_STREAM)
            },
        )
        Intent.ACTION_SEND_MULTIPLE ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java).orEmpty()
            } else {
                intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM).orEmpty()
            }
        else -> emptyList()
    }
}
