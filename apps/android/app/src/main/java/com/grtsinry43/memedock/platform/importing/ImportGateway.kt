package com.grtsinry43.memedock.platform.importing

data class ImportCandidate(val uri: String, val name: String, val size: Long?)
interface ImportGateway {
    suspend fun describe(uri: String): ImportCandidate
    suspend fun copy(candidate: ImportCandidate, destination: String, cancelled: () -> Boolean, progress: (Long) -> Unit)
    suspend fun cancelActiveRead()
}
