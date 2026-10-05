package com.grtsinry43.memedock.feature.trash

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

data class TrashState(val items: List<LibraryItem> = emptyList(), val collections: List<LibraryCollection> = emptyList(),
    val tags: List<LibraryTag> = emptyList(), val loading: Boolean = true, val loadingMore: Boolean = false,
    val hasMore: Boolean = false, val busy: Boolean = false, val error: String? = null)

class TrashViewModel(private val library: LibraryRepository, private val management: ManagementRepository) : ViewModel() {
    private val mutable = MutableStateFlow(TrashState())
    val state = mutable.asStateFlow()
    private var cursor: PageCursor? = null
    private var query: Job? = null
    private var generation = 0
    init { refresh(); viewModelScope.launch { library.changes.filter { it !is LibraryChange.Thumbnail }.collect { refresh() } } }
    fun refresh() {
        generation++
        val request = generation
        query?.cancel(); cursor?.close(); cursor = null
        mutable.update { it.copy(loading = true, loadingMore = false, hasMore = false, error = null) }
        query = viewModelScope.launch {
            try {
                val collections = management.collections(true)
                val tags = management.tags(true)
                val page = library.page("", deleted = true)
                if (request != generation) { page.next?.close(); return@launch }
                cursor = page.next
                mutable.update { it.copy(items = page.items, collections = collections, tags = tags, loading = false, hasMore = page.next != null) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { mutable.update { it.copy(loading = false, error = (failure as? LibraryFailure)?.reason ?: "INTERNAL") } }
        }
    }
    fun more() {
        val next = cursor ?: return
        if (mutable.value.loading || mutable.value.loadingMore) return
        val request = generation
        mutable.update { it.copy(loadingMore = true, error = null) }
        query = viewModelScope.launch {
            try {
                val page = library.page("", next, deleted = true)
                if (request != generation) { page.next?.close(); return@launch }
                next.close(); cursor = page.next
                mutable.update { it.copy(items = (it.items + page.items).distinctBy { value -> value.id }, loadingMore = false, hasMore = page.next != null) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { mutable.update { it.copy(loadingMore = false, error = (failure as? LibraryFailure)?.reason ?: "INTERNAL") } }
        }
    }
    fun restore(action: suspend () -> Unit) {
        if (mutable.value.busy) return
        mutable.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            try { action(); refresh() }
            catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { mutable.update { it.copy(error = (failure as? LibraryFailure)?.reason ?: "INTERNAL") } }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }
    override fun onCleared() { cursor?.close(); super.onCleared() }
}
