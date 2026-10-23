package com.grtsinry43.memedock.data.library

import com.grtsinry43.memedock.app.LibraryBinding
import kotlinx.coroutines.flow.Flow

enum class TrashKind { Sticker, Collection, Tag }
data class TrashEntry(val kind: TrashKind, val id: String, val generation: Long, val deletedRevision: Long)
fun LibraryItem.trashEntry() = TrashEntry(TrashKind.Sticker, id, generation, revision)
fun StickerDetails.trashEntry() = TrashEntry(TrashKind.Sticker, id, generation, revision)
fun LibraryCollection.trashEntry() = TrashEntry(TrashKind.Collection, id, generation, revision)
fun LibraryTag.trashEntry() = TrashEntry(TrashKind.Tag, id, generation, revision)

interface TrashRepository {
    val bindings: Flow<LibraryBinding?>
    suspend fun binding(): LibraryBinding
    suspend fun snapshot(binding: LibraryBinding): List<TrashEntry>
    suspend fun purge(binding: LibraryBinding, entry: TrashEntry)
    suspend fun cleanup(binding: LibraryBinding)
}
