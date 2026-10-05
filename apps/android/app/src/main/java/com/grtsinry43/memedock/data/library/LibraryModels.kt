package com.grtsinry43.memedock.data.library

enum class ThumbnailState { Missing, Generating, Ready, Failed }
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
