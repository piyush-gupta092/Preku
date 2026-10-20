package app.preku.bridge

import app.preku.bridge.protocol.Envelope
import app.preku.bridge.protocol.MessageType
import app.preku.bridge.protocol.PROTOCOL_VERSION
import java.net.URI
import java.net.URLDecoder
import java.security.MessageDigest
import java.util.Locale
import java.util.UUID
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

// The pure parts of the phone bridge (Architecture.md §4): pairing links, fingerprints,
// reconnect timing and how the phone reacts to the PC's messages. All unit-tested.

/** A paired PC, as remembered on the phone. */
@Serializable
data class PairedPc(
    val pcId: String,
    val pcName: String,
    /** SHA-256 of the PC's certificate (lowercase hex): the only certificate accepted. */
    val certFp: String,
    val host: String,
    val port: Int,
)

/** What the pairing QR carries: `preku://pair?host=…&port=…&fp=…&token=…&id=…&name=…`. */
data class PairingLink(val pc: PairedPc, val token: String) {
    companion object {
        private val FP = Regex("^[0-9a-f]{64}$")

        /** Null when the text isn't a valid Preku pairing link. */
        fun parse(text: String): PairingLink? = runCatching {
            val uri = URI(text.trim())
            if (uri.scheme != "preku" || uri.host != "pair") return null
            val query = uri.rawQuery?.split('&').orEmpty().associate { part ->
                val (k, v) = (part.split('=', limit = 2) + "").take(2)
                k to URLDecoder.decode(v, "UTF-8")
            }
            val fp = query["fp"]?.lowercase()?.takeIf { FP.matches(it) } ?: return null
            val port = query["port"]?.toIntOrNull()?.takeIf { it in 1..65535 } ?: return null
            val host = query["host"]?.takeIf { it.isNotBlank() } ?: return null
            val token = query["token"]?.takeIf { it.length >= 16 } ?: return null
            val id = query["id"]?.takeIf { it.isNotBlank() } ?: return null
            PairingLink(PairedPc(id, query["name"]?.ifBlank { null } ?: "PC", fp, host, port), token)
        }.getOrNull()
    }
}

/** SHA-256 of a DER certificate, lowercase hex (same as the desktop's `identity::fingerprint`). */
fun fingerprint(der: ByteArray): String =
    MessageDigest.getInstance("SHA-256").digest(der).joinToString("") { String.format(Locale.ROOT, "%02x", it) }

/** Reconnect delays: 1 s, 2 s, 4 s … capped at 30 s; reset after a good connection. */
class Backoff(private val startMs: Long = 1_000, private val maxMs: Long = 30_000) {
    private var next = startMs

    fun nextDelay(): Long = next.also { next = (next * 2).coerceAtMost(maxMs) }

    fun reset() {
        next = startMs
    }
}

/** How the phone reacts to a message from the PC. */
sealed interface Reaction {
    /** Pairing accepted: remember the PC (its id and name from the message). */
    data class Paired(val pcId: String, val pcName: String) : Reaction
    data object Connected : Reaction
    /** The PC refused; `forget` when it no longer knows this phone. */
    data class Rejected(val reason: String, val forget: Boolean) : Reaction
    /** The PC unpaired this phone. */
    data object Unpaired : Reaction
    data class ReplyPong(val replyTo: String) : Reaction
    data object Ignore : Reaction
}

fun react(envelope: Envelope): Reaction {
    fun text(key: String) = envelope.body[key]?.jsonPrimitive?.content.orEmpty()
    return when (envelope.type) {
        MessageType.PAIR_ACCEPTED -> Reaction.Paired(text("pcId"), text("pcName"))
        MessageType.WELCOME -> Reaction.Connected
        MessageType.PAIR_REJECTED -> text("reason").let { Reaction.Rejected(it, forget = it == "unknown_device") }
        MessageType.UNPAIR -> Reaction.Unpaired
        "system.ping" -> Reaction.ReplyPong(envelope.id)
        else -> Reaction.Ignore
    }
}

val BridgeJson = Json { ignoreUnknownKeys = true; encodeDefaults = false }

/** A protocol envelope as JSON text. */
fun <T> envelopeJson(type: String, serializer: KSerializer<T>, body: T, now: Long = System.currentTimeMillis()): String {
    val element = BridgeJson.encodeToJsonElement(serializer, body)
    return BridgeJson.encodeToString(
        Envelope.serializer(),
        Envelope(PROTOCOL_VERSION, UUID.randomUUID().toString(), type, now, element.jsonObject),
    )
}

fun emptyEnvelopeJson(type: String, now: Long = System.currentTimeMillis()): String =
    BridgeJson.encodeToString(
        Envelope.serializer(),
        Envelope(PROTOCOL_VERSION, UUID.randomUUID().toString(), type, now, JsonObject(emptyMap())),
    )

fun parseEnvelope(text: String): Envelope? =
    runCatching { BridgeJson.decodeFromString(Envelope.serializer(), text) }.getOrNull()
