package com.grtsinry43.memedock.data.library

enum class ThumbnailState { Missing, Generating, Ready, Failed }
/** Explicit page order. Never-used stickers trail [LastUsed], newest created first. */
enum class LibrarySort { Added, LastUsed }
data class LibraryCollection(val id: String, val name: String, val generation: Long = 0, val revision: Long = 0, val deletedAt: Long? = null)
data class LibraryTag(val id: String, val name: String, val generation: Long = 0, val revision: Long = 0, val deletedAt: Long? = null)
data class LibraryStatistics(
    val originalCount: Long,
    val savedOriginalBytes: Long,
    val thumbnailBytes: Long,
    val temporaryShareBytes: Long,
) {
    val cachedBytes: Long get() = thumbnailBytes + temporaryShareBytes
}
data class LibraryItem(
    val id: String,
    val title: String,
    val originalName: String,
    val animated: Boolean,
    val mime: String,
    val width: Int,
    val height: Int,
    val thumbnailState: ThumbnailState,
    val thumbnailPath: String?,
    val error: String?,
    val starred: Boolean = false,
    val generation: Long = 0,
    val revision: Long = 0,
    val deleted: Boolean = false,
)
interface PageCursor : AutoCloseable
data class LibraryPage(val items: List<LibraryItem>, val next: PageCursor?)
sealed interface LibraryChange {
    data object Content : LibraryChange
    data class Thumbnail(val id: String) : LibraryChange
    data object Reload : LibraryChange
}
data class ThumbnailUpdate(val id: String, val state: ThumbnailState, val path: String?, val error: String?)
enum class ImportDisposition { Created, Reused, RestoreRequired }
data class ImportedItem(val id: String, val disposition: ImportDisposition)
interface ImportSlot : AutoCloseable { val path: String }
class LibraryFailure(val reason: String, cause: Throwable? = null) : Exception(reason, cause)

data class StickerDetails(val id: String, val title: String, val note: String, val originalName: String,
    val mime: String, val width: Int, val height: Int, val byteSize: Long, val animated: Boolean,
    val deleted: Boolean, val tags: List<LibraryTag>, val collection: LibraryCollection?, val previewPath: String?, val originalError: String?,
    val starred: Boolean = false, val generation: Long = 0, val revision: Long = 0)
data class RestoreSuggestions(val collection: LibraryCollection?, val tags: List<LibraryTag>)
data class ShareArtifact(val id: String, val path: String, val mime: String, val fileName: String, val byteSize: Long, val animated: Boolean)
interface OutputLease : AutoCloseable

data class CollectionSummary(val collection: LibraryCollection, val count: Long, val coverId: String?, val thumbnailPath: String?)
sealed interface BatchAction {
    data class Assign(val collection: LibraryCollection) : BatchAction
    data class ClearCollection(val expected: LibraryCollection? = null) : BatchAction
    data class AddTags(val tags: List<LibraryTag>) : BatchAction
    data class RemoveTags(val tags: List<LibraryTag>) : BatchAction
    data class Star(val value: Boolean) : BatchAction
    data object Delete : BatchAction
    data object Restore : BatchAction
}
enum class BatchOutcome { Pending, Applied, Unchanged, Failed }
data class BatchItemResult(val id: String, val outcome: BatchOutcome, val error: String?)
data class BatchReport(val items: List<BatchItemResult>, val stopped: Boolean)
interface BatchOperation : AutoCloseable {
    suspend fun awaitResult(): BatchReport
    fun cancel()
    fun snapshot(): BatchReport
}
