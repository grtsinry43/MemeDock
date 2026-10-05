package com.grtsinry43.memedock.feature.collections

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

data class CollectionsState(val items: List<LibraryCollection> = emptyList(), val loading: Boolean = true, val error: String? = null)

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
            try { mutable.value = CollectionsState(repository.collections(), loading = false) }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(loading = false, error = (error as? LibraryFailure)?.reason ?: "INTERNAL") } }
        }
    }
    fun retry() { repository.retryOpen(); refresh() }
}
