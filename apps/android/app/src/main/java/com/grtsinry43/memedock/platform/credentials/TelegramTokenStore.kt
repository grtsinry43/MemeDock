package com.grtsinry43.memedock.platform.credentials

import android.content.Context

interface TelegramTokenStore {
    suspend fun load(): String?
    suspend fun save(token: String)
    suspend fun clear()
}

class AndroidTelegramTokenStore(context: Context) : TelegramTokenStore {
    // Preserve the released credential's filename, alias and ciphertext format.
    private val store = EncryptedCredentialStore(context, "telegram-token", "memedock.telegram.token")
    override suspend fun load() = store.load()
    override suspend fun save(token: String) = store.save(token)
    override suspend fun clear() = store.clear()
}
