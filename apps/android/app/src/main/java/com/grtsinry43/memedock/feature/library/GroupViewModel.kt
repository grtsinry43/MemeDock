package com.grtsinry43.memedock.feature.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*

/** A collection or tag whose stickers fill a page. */
sealed interface StickerGroup {
    val id: String
    data class Collection(override val id: String) : StickerGroup
    data class Tag(override val id: String) : StickerGroup
}

data class GroupState(val name: String, val renaming: Boolean = false, val busy: Boolean = false, val editError: String? = null)

sealed interface GroupEvent {
    data object Deleted : GroupEvent
    data class Failed(val reason: String) : GroupEvent
}

/** Keeps the page's collection or tag current and runs its rename and delete. [name] shows until the first load. */
class GroupViewModel(
    private val repository: ManagementRepository,
    changes: Flow<LibraryChange>,
    private val group: StickerGroup,
    name: String,
) : ViewModel() {
    private val mutable = MutableStateFlow(GroupState(name))
    val state = mutable.asStateFlow()
    private val eventChannel = Channel<GroupEvent>(Channel.BUFFERED)
    val events = eventChannel.receiveAsFlow()
    private var collection: LibraryCollection? = null
    private var tag: LibraryTag? = null
    private var loadError: String? = null
    private var job: Job? = null

    init {
        refresh()
        viewModelScope.launch { changes.filter { it !is LibraryChange.Thumbnail }.collect { refresh() } }
    }

    private fun refresh() {
        job?.cancel()
        job = viewModelScope.launch {
            try {
                val current = when (group) {
                    is StickerGroup.Collection -> repository.collections(false).firstOrNull { it.id == group.id }
                        .also { collection = it }?.name
                    is StickerGroup.Tag -> repository.tags().firstOrNull { it.id == group.id }.also { tag = it }?.name
                }
                loadError = if (current == null) "ENTITY_DELETED" else null
                if (current != null) mutable.update { it.copy(name = current) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { loadError = reason(error) }
        }
    }

    fun startRename() { if (!mutable.value.busy) mutable.update { it.copy(renaming = true, editError = null) } }
    fun cancelRename() { if (!mutable.value.busy) mutable.update { it.copy(renaming = false, editError = null) } }

    fun rename(name: String) = manage(inline = true) {
        when (group) {
            is StickerGroup.Collection -> repository.renameCollection(current(collection), name)
            is StickerGroup.Tag -> repository.renameTag(current(tag), name)
        }
        mutable.update { it.copy(name = name, renaming = false) }
    }

    fun delete() = manage(inline = false) {
        when (group) {
            is StickerGroup.Collection -> repository.deleteCollection(current(collection))
            is StickerGroup.Tag -> repository.deleteTag(current(tag))
        }
        eventChannel.send(GroupEvent.Deleted)
    }

    /** Moves [item] in front of [before], or to the end; collections only. */
    suspend fun moveItem(item: LibraryItem, before: LibraryItem?) =
        repository.moveCollectionItem(current(collection), item, before)

    private fun <T : Any> current(value: T?): T = value ?: throw LibraryFailure(loadError ?: "ENTITY_DELETED")

    /** Inline failures stay in the field being edited; others become a message. */
    private fun manage(inline: Boolean, action: suspend () -> Unit) {
        if (mutable.value.busy) return
        mutable.update { it.copy(busy = true, editError = null) }
        viewModelScope.launch {
            try { action() }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) {
                if (inline) mutable.update { it.copy(editError = reason(error)) }
                else eventChannel.send(GroupEvent.Failed(reason(error)))
            }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }

    private fun reason(error: Exception) = (error as? LibraryFailure)?.reason ?: "INTERNAL"
}
