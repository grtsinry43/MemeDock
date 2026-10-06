package com.grtsinry43.memedock.feature.organize

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/** The one field the page edits in place, if any. */
sealed interface OrganizeEdit {
    data object NewCollection : OrganizeEdit
    data object NewTag : OrganizeEdit
}

/** An existing collection or tag; editing one renames it. */
sealed interface OrganizeItem : OrganizeEdit {
    val name: String
    data class Collection(val value: LibraryCollection) : OrganizeItem { override val name get() = value.name }
    data class Tag(val value: LibraryTag) : OrganizeItem { override val name get() = value.name }
}

data class OrganizeState(
    val collections: List<LibraryCollection> = emptyList(),
    val tags: List<LibraryTag> = emptyList(),
    val loading: Boolean = true,
    val error: String? = null,
    val editing: OrganizeEdit? = null,
    val busy: Boolean = false,
    val editError: String? = null,
)

sealed interface OrganizeEvent {
    data class Deleted(val tag: Boolean) : OrganizeEvent
    data class Failed(val reason: String) : OrganizeEvent
}

class OrganizeViewModel(private val repository: ManagementRepository, changes: Flow<LibraryChange>) : ViewModel() {
    private val mutable = MutableStateFlow(OrganizeState())
    val state = mutable.asStateFlow()
    private val eventChannel = Channel<OrganizeEvent>(Channel.BUFFERED)
    val events = eventChannel.receiveAsFlow()
    private val moves = Mutex()
    private var job: Job? = null

    init {
        refresh()
        viewModelScope.launch { changes.filter { it !is LibraryChange.Thumbnail }.collect { refresh() } }
    }

    fun refresh() {
        job?.cancel()
        job = viewModelScope.launch {
            mutable.update { it.copy(error = null) }
            try {
                val collections = repository.collections(false)
                val tags = repository.tags()
                mutable.update { it.copy(collections = collections, tags = tags, loading = false) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) {
                // A page already on screen stays; only a first load turns into an error state.
                if (mutable.value.loading) mutable.update { it.copy(loading = false, error = reason(error)) }
                else eventChannel.send(OrganizeEvent.Failed(reason(error)))
            }
        }
    }

    fun retry() { mutable.update { it.copy(loading = true, error = null) }; refresh() }

    fun edit(target: OrganizeEdit?) { if (!mutable.value.busy) mutable.update { it.copy(editing = target, editError = null) } }

    /** Creates or renames whatever is being edited; the field closes once the change is saved. */
    fun commit(name: String) {
        val target = mutable.value.editing ?: return
        manage(inline = true) {
            when (target) {
                OrganizeEdit.NewCollection -> repository.createCollection(name)
                OrganizeEdit.NewTag -> repository.createTag(name)
                is OrganizeItem.Collection -> repository.renameCollection(target.value, name)
                is OrganizeItem.Tag -> repository.renameTag(target.value, name)
            }
            mutable.update { it.copy(editing = null) }
        }
    }

    fun delete(item: OrganizeItem) = manage(inline = false) {
        when (item) {
            is OrganizeItem.Collection -> repository.deleteCollection(item.value)
            is OrganizeItem.Tag -> repository.deleteTag(item.value)
        }
        eventChannel.send(OrganizeEvent.Deleted(tag = item is OrganizeItem.Tag))
    }

    /** Shows the new order at once; moves save one at a time and a failure reloads the saved order. */
    fun moveCollection(from: Int, to: Int) {
        val current = mutable.value.collections
        if (from == to || from !in current.indices || to !in current.indices) return
        val reordered = current.toMutableList().apply { add(to, removeAt(from)) }
        val value = reordered[to]
        val before = reordered.getOrNull(to + 1)
        mutable.update { it.copy(collections = reordered) }
        viewModelScope.launch {
            moves.withLock {
                try { repository.moveCollection(value, before) }
                catch (cancel: CancellationException) { throw cancel }
                catch (error: Exception) {
                    eventChannel.send(OrganizeEvent.Failed(reason(error)))
                    refresh()
                }
            }
        }
    }

    /** Inline failures stay in the field being edited; others become a message. */
    private fun manage(inline: Boolean, action: suspend () -> Unit) {
        if (mutable.value.busy) return
        mutable.update { it.copy(busy = true, editError = null) }
        viewModelScope.launch {
            try { action() }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) {
                if (inline) mutable.update { it.copy(editError = reason(error)) }
                else eventChannel.send(OrganizeEvent.Failed(reason(error)))
            }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }

    private fun reason(error: Exception) = (error as? LibraryFailure)?.reason ?: "INTERNAL"
}
