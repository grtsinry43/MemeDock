package com.grtsinry43.memedock.feature.trash

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*

/** Deleted collections and tags; deleted stickers page through the shared grid model. */
data class TrashState(val collections: List<LibraryCollection> = emptyList(), val tags: List<LibraryTag> = emptyList(),
    val restoring: String? = null)

sealed interface TrashEvent {
    data object Restored : TrashEvent
    data class Failed(val reason: String) : TrashEvent
}

class TrashViewModel(private val management: ManagementRepository, changes: Flow<LibraryChange>) : ViewModel() {
    private val mutable = MutableStateFlow(TrashState())
    val state = mutable.asStateFlow()
    private val eventChannel = Channel<TrashEvent>(Channel.BUFFERED)
    val events = eventChannel.receiveAsFlow()
    private var query: Job? = null

    init { refresh(); viewModelScope.launch { changes.filter { it !is LibraryChange.Thumbnail }.collect { refresh() } } }

    fun refresh() {
        query?.cancel()
        query = viewModelScope.launch {
            try {
                val collections = management.collections(true)
                val tags = management.tags(true)
                mutable.update { it.copy(collections = collections, tags = tags) }
            } catch (cancel: CancellationException) { throw cancel }
            // The sticker grid reports library failures; these lists simply stay as they were.
            catch (_: Exception) {}
        }
    }

    fun restore(value: LibraryCollection) = restore(value.id) { management.restoreCollection(value) }
    fun restore(value: LibraryTag) = restore(value.id) { management.restoreTag(value) }

    private fun restore(id: String, action: suspend () -> Unit) {
        if (mutable.value.restoring != null) return
        mutable.update { it.copy(restoring = id) }
        viewModelScope.launch {
            try { action(); eventChannel.send(TrashEvent.Restored) }
            catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { eventChannel.send(TrashEvent.Failed((failure as? LibraryFailure)?.reason ?: "INTERNAL")) }
            finally { mutable.update { it.copy(restoring = null) } }
        }
    }
}
