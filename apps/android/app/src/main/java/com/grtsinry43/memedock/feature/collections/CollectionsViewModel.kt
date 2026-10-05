package com.grtsinry43.memedock.feature.collections

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

data class CollectionsState(val items: List<LibraryCollection> = emptyList(), val loading: Boolean = true, val error: String? = null,
    val busy: Boolean = false, val actionError: String? = null)

class CollectionsViewModel(private val repository: CollectionRepository, changes: Flow<LibraryChange>) : ViewModel() {
    private val mutable = MutableStateFlow(CollectionsState())
    val state = mutable.asStateFlow()
    private var job: Job? = null
    init {
        refresh()
        viewModelScope.launch { changes.filter { it !is LibraryChange.Thumbnail }.collect { refresh() } }
    }
    fun refresh() {
        job?.cancel()
        job = viewModelScope.launch {
            mutable.update { it.copy(loading = true, error = null) }
            try { val items = repository.collections(); mutable.update { it.copy(items = items, loading = false) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(loading = false, error = (error as? LibraryFailure)?.reason ?: "INTERNAL") } }
        }
    }
    fun retry() { repository.retryOpen(); refresh() }
    fun manage(action: suspend () -> Unit, success: () -> Unit) {
        if (mutable.value.busy) return
        mutable.update { it.copy(busy = true, actionError = null) }
        viewModelScope.launch {
            try { action(); success(); refresh() }
            catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { mutable.update { it.copy(actionError = (failure as? LibraryFailure)?.reason ?: "INTERNAL") } }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }
}
