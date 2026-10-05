package com.grtsinry43.memedock.data.library

interface ManagementRepository {
    suspend fun patchSticker(detail: StickerDetails, title: String? = null, note: String? = null, starred: Boolean? = null)
    suspend fun relations(detail: StickerDetails, collections: List<LibraryCollection>?, tags: List<LibraryTag>?)
    suspend fun createCollection(name: String): LibraryCollection
    suspend fun renameCollection(value: LibraryCollection, name: String)
    suspend fun deleteCollection(value: LibraryCollection)
    suspend fun moveCollection(value: LibraryCollection, before: LibraryCollection?)
    suspend fun moveCollectionItem(collection: LibraryCollection, value: LibraryItem, before: LibraryItem?)
    suspend fun collections(deleted: Boolean): List<LibraryCollection>
    suspend fun tags(deleted: Boolean = false): List<LibraryTag>
    suspend fun createTag(name: String): LibraryTag
    suspend fun renameTag(value: LibraryTag, name: String)
    suspend fun deleteTag(value: LibraryTag)
    suspend fun deleteSticker(value: StickerDetails)
    suspend fun restoreSticker(value: StickerDetails)
    suspend fun restoreCollection(value: LibraryCollection)
    suspend fun restoreTag(value: LibraryTag)
    suspend fun suggestions(id: String): RestoreSuggestions
}
