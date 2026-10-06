package com.grtsinry43.memedock.data.library

enum class ThumbnailState { Missing, Generating, Ready, Failed }
/** Explicit page order. Never-used stickers trail [LastUsed], newest created first. */
enum class LibrarySort { Added, LastUsed }
data class LibraryCollection(val id: String, val name: String, val generation: Long = 0, val revision: Long = 0, val deletedAt: Long? = null)
data class LibraryTag(val id: String, val name: String, val generation: Long = 0, val revision: Long = 0, val deletedAt: Long? = null)
data class LibraryStatistics(val originalCount: Long, val savedOriginalBytes: Long)
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
    val deleted: Boolean, val tags: List<LibraryTag>, val collections: List<LibraryCollection>, val previewPath: String?, val originalError: String?,
    val starred: Boolean = false, val generation: Long = 0, val revision: Long = 0)
data class RestoreSuggestions(val collections: List<LibraryCollection>, val tags: List<LibraryTag>)
data class ShareArtifact(val id: String, val path: String, val mime: String, val fileName: String, val byteSize: Long, val animated: Boolean)
interface OutputLease : AutoCloseable
