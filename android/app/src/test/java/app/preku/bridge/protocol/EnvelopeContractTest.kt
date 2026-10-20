package app.preku.bridge.protocol

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/** Contract test: every shared example in /protocol/examples must parse and round-trip. */
class EnvelopeContractTest {
    // Gradle runs unit tests with the module directory (android/app) as working directory.
    private val examplesDir = File("../../protocol/examples")

    @Test
    fun parsesAllSharedExamples() {
        val files = examplesDir.listFiles { file -> file.extension == "json" }.orEmpty()
        assertTrue("no examples found in ${examplesDir.absolutePath}", files.isNotEmpty())

        for (file in files) {
            val raw = file.readText()
            val envelope = Json.decodeFromString(Envelope.serializer(), raw)
            assertEquals(file.name, PROTOCOL_VERSION, envelope.v)
            assertEquals(
                file.name,
                Json.parseToJsonElement(raw),
                Json.encodeToJsonElement(Envelope.serializer(), envelope),
            )
        }
    }
}
