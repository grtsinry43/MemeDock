package com.grtsinry43.memedock.platform.credentials

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileNotFoundException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

interface TelegramTokenStore {
    suspend fun load(): String?
    suspend fun save(token: String)
    suspend fun clear()
}

/** Only ciphertext lives on disk; credentials are excluded from both library and Android backups. */
class AndroidTelegramTokenStore(context: Context) : TelegramTokenStore {
    private val file = AtomicFile(File(context.noBackupFilesDir, "telegram-token"))
    private val mutex = Mutex()
    private val alias = "memedock.telegram.token"
    private fun store() = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    override suspend fun load(): String? = withContext(Dispatchers.IO) { mutex.withLock {
        val input = try { file.openRead() } catch (_: FileNotFoundException) { return@withLock null }
        val bytes = input.use { stream ->
            val buffer = ByteArray(1025)
            var length = 0
            while (length < buffer.size) {
                val count = stream.read(buffer, length, buffer.size - length)
                if (count < 0) break
                length += count
            }
            require(length in 29..1024)
            buffer.copyOf(length)
        }
        try {
            val key = store().getKey(alias, null) as? SecretKey ?: error("Credential key unavailable")
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, bytes.copyOfRange(0, 12)))
            val clear = cipher.doFinal(bytes, 12, bytes.size - 12)
            try { clear.toString(Charsets.UTF_8) } finally { clear.fill(0) }
        } finally { bytes.fill(0) }
    } }
    override suspend fun save(token: String) = withContext(Dispatchers.IO) { mutex.withLock {
        require(token.isNotBlank() && token.length <= 256)
        val keys = store()
        val key = (keys.getKey(alias, null) as? SecretKey) ?: KeyGenerator.getInstance("AES", "AndroidKeyStore").run {
            init(KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256).build())
            generateKey()
        }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key)
        val clear = token.toByteArray(Charsets.UTF_8)
        val encrypted = try { cipher.doFinal(clear) } finally { clear.fill(0) }
        check(cipher.iv.size == 12)
        val output = file.startWrite()
        try {
            output.write(cipher.iv); output.write(encrypted); file.finishWrite(output)
        } catch (error: Exception) { file.failWrite(output); throw error }
        finally { encrypted.fill(0) }
    } }
    override suspend fun clear() = withContext(Dispatchers.IO) { mutex.withLock {
        file.delete()
        check(!file.baseFile.exists())
        store().deleteEntry(alias)
    } }
}
