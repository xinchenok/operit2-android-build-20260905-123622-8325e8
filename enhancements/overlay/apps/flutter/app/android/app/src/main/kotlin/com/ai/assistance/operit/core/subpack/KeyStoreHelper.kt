package com.ai.assistance.operit.core.subpack

import android.content.Context
import java.io.File
import java.math.BigInteger
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.SecureRandom
import java.security.Signature
import java.security.cert.CertificateFactory
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/** Exact compatibility entry used by the bundled APK reverse helper's debug-sign command. */
object KeyStoreHelper {
    /** This is a device-local APK-tool debug key, unrelated to the application's release signature. */
    @JvmStatic
    @Synchronized
    fun getOrCreateKeystore(context: Context): File {
        val target = File(context.filesDir, "legacy-apktool-debug.p12")
        val password = "android".toCharArray()
        if (target.isFile) {
            val existing = KeyStore.getInstance("PKCS12")
            target.inputStream().use { existing.load(it, password) }
            require(existing.isKeyEntry("androidkey")) { "APK tool debug keystore is missing androidkey" }
            return target
        }
        val generator = KeyPairGenerator.getInstance("RSA").apply { initialize(2048) }
        val key = generator.generateKeyPair()
        val algorithm = sequence(der(6, byteArrayOf(42, -122, 72, -122, -9, 13, 1, 1, 11)), der(5, byteArrayOf()))
        val subject = sequence(der(49, sequence(der(6, byteArrayOf(85, 4, 3)), der(12, "Operit APK Tool Debug".toByteArray()))))
        val serial = der(2, BigInteger(128, SecureRandom()).abs().add(BigInteger.ONE).toByteArray())
        val now = System.currentTimeMillis()
        val validity = sequence(time(now - 86_400_000L), time(now + 25L * 365 * 86_400_000L))
        val body = sequence(der(160, der(2, byteArrayOf(2))), serial, algorithm, subject, validity, subject, key.public.encoded)
        val signer = Signature.getInstance("SHA256withRSA").apply { initSign(key.private); update(body) }
        val certificateBytes = sequence(body, algorithm, der(3, byteArrayOf(0) + signer.sign()))
        val certificate = CertificateFactory.getInstance("X.509").generateCertificate(certificateBytes.inputStream())
        val store = KeyStore.getInstance("PKCS12").apply {
            load(null, password)
            setKeyEntry("androidkey", key.private, password, arrayOf(certificate))
        }
        val staging = File(context.filesDir, "legacy-apktool-debug.p12.tmp")
        staging.outputStream().use { store.store(it, password); it.fd.sync() }
        require(staging.renameTo(target)) { "Cannot save APK tool debug keystore" }
        return target
    }
    private fun time(timestamp: Long): ByteArray {
        val format = SimpleDateFormat("yyyyMMddHHmmss'Z'", Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }
        return der(24, format.format(Date(timestamp)).toByteArray(Charsets.US_ASCII))
    }
    private fun sequence(vararg values: ByteArray): ByteArray = der(48, values.fold(byteArrayOf()) { all, value -> all + value })
    private fun der(tag: Int, value: ByteArray): ByteArray {
        val size = value.size
        val length = if (size < 128) byteArrayOf(size.toByte()) else {
            var rest = size
            var bytes = byteArrayOf()
            while (rest > 0) { bytes = byteArrayOf(rest.toByte()) + bytes; rest = rest ushr 8 }
            byteArrayOf((128 + bytes.size).toByte()) + bytes
        }
        return byteArrayOf(tag.toByte()) + length + value
    }
}
