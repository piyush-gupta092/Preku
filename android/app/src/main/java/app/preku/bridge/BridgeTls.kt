package app.preku.bridge

import android.annotation.SuppressLint
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.math.BigInteger
import java.net.Socket
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.Principal
import java.security.PrivateKey
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec
import java.util.Calendar
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLEngine
import javax.net.ssl.X509ExtendedKeyManager
import javax.net.ssl.X509TrustManager
import javax.security.auth.x500.X500Principal
import okhttp3.OkHttpClient

/**
 * The phone's bridge identity: an EC P-256 key kept in the Android Keystore (it never leaves the
 * secure hardware), with the self-signed certificate the Keystore makes for it. The PC pins this
 * certificate's fingerprint at pairing.
 */
object PhoneIdentity {
    private const val ALIAS = "preku_bridge"
    private const val KEYSTORE = "AndroidKeyStore"

    private fun keyStore(): KeyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }

    /** Creates the key on first use. */
    fun ensure() {
        if (keyStore().containsAlias(ALIAS)) return
        val start = Calendar.getInstance()
        val end = Calendar.getInstance().apply { add(Calendar.YEAR, 100) }
        val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY)
            .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_NONE)
            .setCertificateSubject(X500Principal("CN=Preku phone"))
            .setCertificateSerialNumber(BigInteger.ONE)
            .setCertificateNotBefore(start.time)
            .setCertificateNotAfter(end.time)
            .build()
        KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, KEYSTORE).apply {
            initialize(spec)
            generateKeyPair()
        }
    }

    fun certificate(): X509Certificate = keyStore().getCertificate(ALIAS) as X509Certificate

    fun privateKey(): PrivateKey = keyStore().getKey(ALIAS, null) as PrivateKey

    /** Forget the identity (after unpairing, a new pairing gets a fresh key). */
    fun delete() {
        runCatching { keyStore().deleteEntry(ALIAS) }
    }

    /** Presents the Keystore certificate as the TLS client certificate. */
    class KeyManager : X509ExtendedKeyManager() {
        override fun chooseClientAlias(keyType: Array<out String>?, issuers: Array<out Principal>?, socket: Socket?) = ALIAS
        override fun chooseEngineClientAlias(keyType: Array<out String>?, issuers: Array<out Principal>?, engine: SSLEngine?) = ALIAS
        override fun getCertificateChain(alias: String?): Array<X509Certificate> = arrayOf(certificate())
        override fun getPrivateKey(alias: String?): PrivateKey = privateKey()
        override fun getClientAliases(keyType: String?, issuers: Array<out Principal>?) = arrayOf(ALIAS)
        override fun getServerAliases(keyType: String?, issuers: Array<out Principal>?): Array<String> = emptyArray()
        override fun chooseServerAlias(keyType: String?, issuers: Array<out Principal>?, socket: Socket?): String? = null
    }
}

/**
 * Trusts exactly one server certificate: the PC whose fingerprint came from the pairing QR.
 * No certificate authorities are involved, so a stranger on the Wi-Fi can't pretend to be the PC.
 */
@SuppressLint("CustomX509TrustManager") // Deliberate: pinning, not trusting everything.
class PinnedTrustManager(private val fingerprint: String) : X509TrustManager {
    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        val leaf = chain?.firstOrNull() ?: throw CertificateException("no certificate")
        if (fingerprint(leaf.encoded) != fingerprint) throw CertificateException("not the paired PC")
    }

    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        throw CertificateException("the phone is never a TLS server")
    }

    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
}

/** An OkHttp client that pins the PC and presents the phone's Keystore certificate. */
@SuppressLint("BadHostnameVerifier") // The pinned certificate fingerprint is the identity check.
fun bridgeHttpClient(pcFingerprint: String): OkHttpClient {
    PhoneIdentity.ensure()
    val trust = PinnedTrustManager(pcFingerprint)
    val tls = SSLContext.getInstance("TLS").apply {
        init(arrayOf(PhoneIdentity.KeyManager()), arrayOf(trust), null)
    }
    return OkHttpClient.Builder()
        .sslSocketFactory(tls.socketFactory, trust)
        // The PC is reached by IP and identified by its pinned certificate, not by a hostname.
        .hostnameVerifier { _, _ -> true }
        .pingInterval(15, TimeUnit.SECONDS)
        .connectTimeout(5, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.SECONDS)
        .build()
}
