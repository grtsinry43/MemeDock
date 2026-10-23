package com.grtsinry43.memedock.data.sync

import com.grtsinry43.memedock.app.LibraryBinding
import kotlinx.coroutines.flow.Flow

data class SyncStatus(
    val origin: String? = null, val deviceId: String? = null,
    val connected: Boolean = false, val paused: Boolean = false,
    val pending: Long = 0, val blocked: Long = 0, val cursor: Long = 0,
    val lastSuccessMs: Long? = null, val error: String? = null, val retryAfterMs: Long = 15_000,
)

data class SyncWorkIdentity(val libraryId: String, val origin: String, val deviceId: String)
fun SyncStatus.workIdentity(binding: LibraryBinding): SyncWorkIdentity? =
    if (connected && origin != null && deviceId != null) SyncWorkIdentity(binding.libraryId, origin, deviceId) else null

interface SyncRepository {
    val bindings: Flow<LibraryBinding?>
    suspend fun binding(): LibraryBinding
    suspend fun status(binding: LibraryBinding): SyncStatus
    suspend fun connect(binding: LibraryBinding, origin: String, token: String): SyncStatus
    suspend fun synchronize(binding: LibraryBinding, token: String): SyncStatus
    suspend fun pause(binding: LibraryBinding, paused: Boolean): SyncStatus
    fun observe(binding: LibraryBinding): Flow<SyncStatus>
}

interface SyncWorkScheduler {
    fun configure(identity: SyncWorkIdentity?, wifiOnly: Boolean)
    fun request(identity: SyncWorkIdentity, wifiOnly: Boolean)
    fun automaticNetworkAvailable(wifiOnly: Boolean): Boolean
}
