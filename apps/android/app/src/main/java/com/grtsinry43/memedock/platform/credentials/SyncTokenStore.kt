package com.grtsinry43.memedock.platform.credentials

import android.content.Context
import java.security.MessageDigest
import java.util.concurrent.ConcurrentHashMap

interface SyncTokenStore {
    suspend fun load(libraryId: String, origin: String): String?
    suspend fun save(libraryId: String, origin: String, token: String)
    suspend fun clear(libraryId: String, origin: String)
}

class AndroidSyncTokenStore(context: Context) : SyncTokenStore {
    private val context = context.applicationContext
    private val stores = ConcurrentHashMap<String, EncryptedCredentialStore>()
    private fun store(libraryId: String, origin: String): EncryptedCredentialStore {
        val key = MessageDigest.getInstance("SHA-256").digest("$libraryId\n$origin".toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it) }
        return stores.getOrPut(key) { EncryptedCredentialStore(context, "sync-$key", "memedock.sync.$key") }
    }
    override suspend fun load(libraryId: String, origin: String) = store(libraryId, origin).load()
    override suspend fun save(libraryId: String, origin: String, token: String) = store(libraryId, origin).save(token)
    override suspend fun clear(libraryId: String, origin: String) = store(libraryId, origin).clear()
}
