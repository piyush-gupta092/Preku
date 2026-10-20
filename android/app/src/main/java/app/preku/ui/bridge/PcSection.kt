package app.preku.ui.bridge

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.preku.R
import app.preku.bridge.BridgeManager
import app.preku.bridge.BridgeServiceController
import app.preku.bridge.BridgeUiState
import app.preku.bridge.LinkStatus
import app.preku.bridge.PairingLink
import app.preku.bridge.Direction
import app.preku.bridge.TransferState
import app.preku.bridge.TransferUi
import app.preku.ui.PrekuCard
import app.preku.ui.SectionTitle
import app.preku.ui.theme.PrekuTheme
import app.preku.ui.theme.Spacing
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.StateFlow

@HiltViewModel
class BridgeViewModel @Inject constructor(
    private val bridge: BridgeManager,
    private val service: BridgeServiceController,
) : ViewModel() {
    val state: StateFlow<BridgeUiState> = bridge.state
    val transfers: StateFlow<List<TransferUi>> = bridge.files.transfers

    fun sendFiles(uris: List<android.net.Uri>) = bridge.files.send(uris)

    fun cancel(id: String) = bridge.files.cancel(id)

    /** Returns false (nothing happens) for text that isn't a pairing link. */
    fun pair(text: String): Boolean {
        val link = PairingLink.parse(text) ?: return false
        bridge.pair(link)
        return true
    }

    fun unpair() {
        bridge.unpair()
    }

    /** Keep the link alive in the background while a PC is paired. */
    fun ensureService() {
        if (bridge.state.value.pc != null) service.ensureRunning()
    }
}

/** Settings → Your PC: pair by scanning the PC's QR (or pasting its link), see status, unpair. */
@Composable
fun PcSection(viewModel: BridgeViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val transfers by viewModel.transfers.collectAsStateWithLifecycle()
    val picker = androidx.activity.compose.rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.OpenMultipleDocuments(),
    ) { uris -> if (uris.isNotEmpty()) viewModel.sendFiles(uris) }
    var link by rememberSaveable { mutableStateOf("") }
    var badLink by rememberSaveable { mutableStateOf(false) }
    val scanner = androidx.activity.compose.rememberLauncherForActivityResult(ScanContract()) { result ->
        result.contents?.let { badLink = !viewModel.pair(it) }
    }
    val scanPrompt = stringResource(R.string.pc_scan_prompt)

    // Paired (now or just now): keep the connection running.
    LaunchedEffect(state.pc != null) { viewModel.ensureService() }

    SectionTitle(stringResource(R.string.pc_title))
    PrekuCard(Modifier.fillMaxWidth(), accent = PrekuTheme.colors.modBridge) {
        val pc = state.pc
        if (pc != null) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.s2)) {
                val online = state.status == LinkStatus.Connected
                Box(
                    Modifier
                        .size(10.dp)
                        .background(if (online) PrekuTheme.colors.success else PrekuTheme.colors.textMuted, CircleShape),
                )
                Column(Modifier.weight(1f)) {
                    Text("💻 ${pc.pcName}", style = MaterialTheme.typography.titleMedium)
                    Text(
                        stringResource(
                            when (state.status) {
                                LinkStatus.Connected -> R.string.pc_connected
                                LinkStatus.Connecting, LinkStatus.Pairing -> R.string.pc_connecting
                                else -> R.string.pc_offline
                            },
                        ),
                        style = MaterialTheme.typography.bodyMedium,
                        color = PrekuTheme.colors.textMuted,
                    )
                }
                OutlinedButton(onClick = viewModel::unpair) { Text(stringResource(R.string.pc_unpair)) }
            }
            // ---- Files ----
            Button(
                onClick = { picker.launch(arrayOf("*/*")) },
                enabled = state.status == LinkStatus.Connected,
                modifier = Modifier.fillMaxWidth(),
            ) { Text(stringResource(R.string.files_send)) }
            transfers.forEach { TransferRow(it, pc.pcName, viewModel) }
            Text(stringResource(R.string.files_saved_in), style = MaterialTheme.typography.labelSmall, color = PrekuTheme.colors.textMuted)
        } else {
            Text(stringResource(R.string.pc_hint), style = MaterialTheme.typography.bodyMedium)
            Button(
                onClick = {
                    scanner.launch(
                        ScanOptions()
                            .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                            .setPrompt(scanPrompt)
                            .setBeepEnabled(false)
                            .setOrientationLocked(false),
                    )
                },
                enabled = state.status != LinkStatus.Pairing,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(stringResource(if (state.status == LinkStatus.Pairing) R.string.pc_pairing else R.string.pc_scan))
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.s2)) {
                OutlinedTextField(
                    value = link,
                    onValueChange = {
                        link = it
                        badLink = false
                    },
                    placeholder = { Text(stringResource(R.string.pc_paste_hint)) },
                    singleLine = true,
                    modifier = Modifier.weight(1f),
                )
                OutlinedButton(onClick = { badLink = !viewModel.pair(link) }) { Text(stringResource(R.string.pc_connect)) }
            }
            if (badLink) {
                Text(stringResource(R.string.pc_bad_link), color = PrekuTheme.colors.danger, style = MaterialTheme.typography.bodyMedium)
            }
        }
        state.problem?.let { problem ->
            Text(problemText(problem), color = PrekuTheme.colors.warning, style = MaterialTheme.typography.bodyMedium)
        }
        Text(stringResource(R.string.pc_privacy), style = MaterialTheme.typography.labelSmall, color = PrekuTheme.colors.textMuted)
    }
}

@Composable
private fun problemText(problem: String): String = stringResource(
    when (problem) {
        "expired" -> R.string.pc_problem_expired
        "invalid" -> R.string.pc_problem_invalid
        "not_pairing" -> R.string.pc_problem_not_pairing
        "unknown_device" -> R.string.pc_problem_unknown_device
        else -> R.string.pc_problem_network
    },
)

@Composable
private fun TransferRow(transfer: TransferUi, pcName: String, viewModel: BridgeViewModel) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val running = transfer.state == TransferState.Active || transfer.state == TransferState.Waiting
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.s2)) {
        Text(if (transfer.direction == Direction.Send) "↑" else "↓", style = MaterialTheme.typography.titleMedium, color = PrekuTheme.colors.modBridge)
        Column(Modifier.weight(1f)) {
            Text(transfer.name, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
            val peer = stringResource(if (transfer.direction == Direction.Send) R.string.files_to else R.string.files_from, pcName)
            val status = when (transfer.state) {
                TransferState.Waiting -> stringResource(R.string.files_waiting)
                TransferState.Active -> stringResource(R.string.files_progress, mb(transfer.bytes), mb(transfer.size))
                TransferState.Done -> stringResource(R.string.files_done)
                TransferState.Failed -> stringResource(R.string.files_failed)
            }
            Text("$status · $peer", style = MaterialTheme.typography.labelSmall, color = PrekuTheme.colors.textMuted)
            if (running) {
                androidx.compose.material3.LinearProgressIndicator(
                    progress = { if (transfer.size > 0) transfer.bytes.toFloat() / transfer.size else 0f },
                    color = PrekuTheme.colors.modBridge,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
        val uri = transfer.uri
        if (running) {
            OutlinedButton(onClick = { viewModel.cancel(transfer.id) }) { Text(stringResource(R.string.files_cancel)) }
        } else if (uri != null && transfer.state == TransferState.Done) {
            OutlinedButton(onClick = {
                runCatching {
                    context.startActivity(
                        android.content.Intent(android.content.Intent.ACTION_VIEW)
                            .setDataAndType(uri, context.contentResolver.getType(uri))
                            .addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION),
                    )
                }
            }) { Text(stringResource(R.string.files_open)) }
        }
    }
}

private fun mb(bytes: Long): String = when {
    bytes < 1024 * 1024 -> "${bytes / 1024} KB"
    else -> String.format(java.util.Locale.ROOT, "%.1f MB", bytes / 1048576.0)
}
