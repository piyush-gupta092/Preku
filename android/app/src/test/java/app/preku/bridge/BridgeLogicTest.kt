package app.preku.bridge

import app.preku.bridge.protocol.DeviceStatus
import app.preku.bridge.protocol.Envelope
import app.preku.bridge.protocol.MessageType
import app.preku.bridge.protocol.PROTOCOL_VERSION
import java.io.File
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BridgeLogicTest {
    private val fp = "a".repeat(64)

    @Test
    fun parsesThePcsPairingLink() {
        val link = PairingLink.parse(
            "preku://pair?host=192.168.1.20&port=47823&fp=${fp.uppercase()}&token=Zm9vYmFyYmF6cXV4MTIzNA&id=pc-1&name=Desk%20PC%20%232",
        )!!
        assertEquals("192.168.1.20", link.pc.host)
        assertEquals(47823, link.pc.port)
        assertEquals(fp, link.pc.certFp)
        assertEquals("Desk PC #2", link.pc.pcName)
        assertEquals("pc-1", link.pc.pcId)
        assertEquals("Zm9vYmFyYmF6cXV4MTIzNA", link.token)
    }

    @Test
    fun rejectsAnythingElse() {
        val good = "preku://pair?host=h&port=1&fp=$fp&token=Zm9vYmFyYmF6cXV4MTIzNA&id=x"
        assertNotNull(PairingLink.parse(good))
        for (bad in listOf(
            "https://example.com",
            good.replace("preku://", "http://"),
            good.replace("fp=$fp", "fp=abc"),
            good.replace("port=1", "port=99999"),
            good.replace("token=Zm9vYmFyYmF6cXV4MTIzNA", "token=short"),
            good.replace("&id=x", ""),
            "not a link at all",
        )) {
            assertNull(bad, PairingLink.parse(bad))
        }
    }

    @Test
    fun fingerprintsMatchTheDesktop() {
        // SHA-256("abc"): the same function the PC uses to pin certificates.
        assertEquals(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            fingerprint("abc".toByteArray()),
        )
    }

    @Test
    fun reconnectDelaysDoubleUpTo30Seconds() {
        val backoff = Backoff()
        assertEquals(listOf(1_000L, 2_000L, 4_000L, 8_000L, 16_000L, 30_000L, 30_000L), List(7) { backoff.nextDelay() })
        backoff.reset()
        assertEquals(1_000L, backoff.nextDelay())
    }

    private fun env(type: String, vararg body: Pair<String, String>) =
        Envelope(PROTOCOL_VERSION, "id-1", type, 0, JsonObject(body.associate { (k, v) -> k to JsonPrimitive(v) }))

    @Test
    fun reactsToThePc() {
        assertEquals(Reaction.Paired("pc-1", "Desk"), react(env(MessageType.PAIR_ACCEPTED, "pcId" to "pc-1", "pcName" to "Desk")))
        assertEquals(Reaction.Connected, react(env(MessageType.WELCOME, "pcId" to "pc-1", "pcName" to "Desk")))
        assertEquals(Reaction.Rejected("unknown_device", forget = true), react(env(MessageType.PAIR_REJECTED, "reason" to "unknown_device")))
        assertEquals(Reaction.Rejected("expired", forget = false), react(env(MessageType.PAIR_REJECTED, "reason" to "expired")))
        assertEquals(Reaction.Unpaired, react(env(MessageType.UNPAIR)))
        assertEquals(Reaction.ReplyPong("id-1"), react(env("system.ping")))
        assertEquals(Reaction.Ignore, react(env("notification.posted")))
    }

    @Test
    fun buildsEnvelopesThePcCanRead() {
        val text = envelopeJson(MessageType.DEVICE_STATUS, DeviceStatus.serializer(), DeviceStatus(82, false, "wifi"), now = 5)
        val back = parseEnvelope(text)!!
        assertEquals(MessageType.DEVICE_STATUS, back.type)
        assertEquals(5L, back.ts)
        assertEquals(JsonPrimitive(82), back.body["battery"])
        assertTrue(parseEnvelope("{nope") == null)
    }

    @Test
    fun everySharedExampleBodyParses() {
        val files = File("../../protocol/examples").listFiles { f -> f.extension == "json" }.orEmpty()
        assertTrue(files.isNotEmpty())
        for (file in files) {
            val envelope = parseEnvelope(file.readText())!!
            val serializer = when (envelope.type) {
                MessageType.PAIR_REQUEST -> app.preku.bridge.protocol.PairRequest.serializer()
                MessageType.PAIR_ACCEPTED -> app.preku.bridge.protocol.PairAccepted.serializer()
                MessageType.PAIR_REJECTED -> app.preku.bridge.protocol.PairRejected.serializer()
                MessageType.HELLO -> app.preku.bridge.protocol.Hello.serializer()
                MessageType.WELCOME -> app.preku.bridge.protocol.Welcome.serializer()
                MessageType.DEVICE_STATUS -> DeviceStatus.serializer()
                MessageType.PONG -> app.preku.bridge.protocol.Pong.serializer()
                MessageType.FILE_OFFER -> app.preku.bridge.protocol.FileOffer.serializer()
                MessageType.FILE_ACCEPT -> app.preku.bridge.protocol.FileAccept.serializer()
                MessageType.FILE_DECLINE -> app.preku.bridge.protocol.FileDecline.serializer()
                MessageType.FILE_CHUNK -> app.preku.bridge.protocol.FileChunk.serializer()
                MessageType.FILE_ACK -> app.preku.bridge.protocol.FileAck.serializer()
                MessageType.FILE_DONE -> app.preku.bridge.protocol.FileDone.serializer()
                MessageType.FILE_RESULT -> app.preku.bridge.protocol.FileResult.serializer()
                MessageType.FILE_CANCEL -> app.preku.bridge.protocol.FileCancel.serializer()
                MessageType.CLIPBOARD_TEXT -> app.preku.bridge.protocol.ClipboardText.serializer()
                MessageType.UNPAIR, "system.ping" -> null
                else -> error("unknown example type ${envelope.type}")
            }
            serializer?.let { BridgeJson.decodeFromJsonElement(it, envelope.body) }
        }
    }

    @Test
    fun namesFromThePcAreMadeSafeLikeOnTheDesktop() {
        assertEquals("holiday.jpg", sanitizeName("holiday.jpg"))
        assertEquals("evil.dll", sanitizeName("../../Windows/system32/evil.dll"))
        assertEquals("a.txt", sanitizeName("C:\\Users\\x\\a.txt"))
        assertEquals("what__is_this_.txt", sanitizeName("what?<is>this*.txt"))
        assertEquals("_CON.txt", sanitizeName("CON.txt"))
        assertEquals("_com1", sanitizeName("com1"))
        assertEquals("compass.txt", sanitizeName("compass.txt"))
        assertEquals("file", sanitizeName("..."))
        assertEquals("hidden", sanitizeName(".hidden"))
        assertEquals(120, sanitizeName("a".repeat(300)).length)
    }

    @Test
    fun countsChunksLikeTheDesktop() {
        assertEquals(1, chunkCount(0))
        assertEquals(1, chunkCount(CHUNK.toLong()))
        assertEquals(2, chunkCount(CHUNK + 1L))
        assertEquals(10, chunkCount(10L * CHUNK))
    }
}
