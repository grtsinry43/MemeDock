package com.grtsinry43.memedock.data.library

import kotlinx.coroutines.flow.Flow

interface LibraryRepository {
    val changes: Flow<LibraryChange>
    /** A null [sort] keeps the natural order: collection order inside a collection, newest first elsewhere. */
    suspend fun page(text: String, cursor: PageCursor? = null, collectionId: String? = null, starred: Boolean? = null,
        deleted: Boolean = false, tagIds: List<String> = emptyList(), sort: LibrarySort? = null): LibraryPage
    suspend fun thumbnail(id: String): String
    suspend fun thumbnailStates(ids: List<String>): List<ThumbnailUpdate>
    suspend fun createInput(): ImportSlot
    suspend fun discardInput(input: ImportSlot)
    suspend fun importInput(input: ImportSlot, name: String, collectionId: String? = null, cancelled: () -> Boolean): ImportedItem
    fun retryOpen()
}

interface DetailRepository {
    suspend fun detail(id: String): StickerDetails
    suspend fun exportOriginal(id: String): OutputLease
    suspend fun export(id: String, choice: com.grtsinry43.memedock.data.settings.ExportChoice, firstFrame: Boolean): OutputLease
    suspend fun prepareHandoff(lease: OutputLease): ShareArtifact
    suspend fun recordShareLaunched(id: String)
    suspend fun recordCopy(id: String)
    suspend fun recordSaved(id: String)
}

interface ClipboardRepository {
    suspend fun protectClipboard(lease: OutputLease): String
    suspend fun reconcileClipboard(observed: String?)
    suspend fun abortClipboard(reference: String)
}

interface CollectionRepository {
    fun retryOpen()
    suspend fun collections(): List<LibraryCollection>
    suspend fun statistics(): LibraryStatistics
}
