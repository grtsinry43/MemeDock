package com.grtsinry43.memedock.data.library

import kotlinx.coroutines.flow.Flow

interface LibraryRepository {
    val changes: Flow<LibraryChange>
    suspend fun page(text: String, cursor: PageCursor? = null, collectionId: String? = null): LibraryPage
    suspend fun thumbnail(id: String): String
    suspend fun thumbnailStates(ids: List<String>): List<ThumbnailUpdate>
    suspend fun createInput(): ImportSlot
    suspend fun discardInput(input: ImportSlot)
    suspend fun importInput(input: ImportSlot, name: String, cancelled: () -> Boolean): ImportedItem
    fun retryOpen()
}

interface DetailRepository {
    suspend fun detail(id: String): StickerDetails
    suspend fun exportOriginal(id: String): OutputLease
    suspend fun prepareHandoff(lease: OutputLease): ShareArtifact
    suspend fun recordShareLaunched(id: String)
}

interface CollectionRepository {
    fun retryOpen()
    suspend fun collections(): List<LibraryCollection>
    suspend fun statistics(): LibraryStatistics
}
