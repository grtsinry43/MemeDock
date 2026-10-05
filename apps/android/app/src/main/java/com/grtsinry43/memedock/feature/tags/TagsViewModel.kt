package com.grtsinry43.memedock.feature.tags

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

data class TagsState(val items: List<LibraryTag> = emptyList(), val loading: Boolean = true,
    val busy: Boolean = false, val error: String? = null)

class TagsViewModel(private val repository: ManagementRepository, changes: Flow<LibraryChange>) : ViewModel() {
    private val mutable = MutableStateFlow(TagsState())
    val state = mutable.asStateFlow()
    private var query: Job? = null
    init { refresh(); viewModelScope.launch { changes.filter { it !is LibraryChange.Thumbnail }.collect { refresh() } } }
    fun refresh() {
        query?.cancel()
        query = viewModelScope.launch {
            mutable.update { it.copy(loading = true, error = null) }
            try { val items = repository.tags(); mutable.update { it.copy(items = items, loading = false) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { mutable.update { it.copy(loading = false, error = (failure as? LibraryFailure)?.reason ?: "INTERNAL") } }
        }
    }
    fun manage(action: suspend () -> Unit, success: () -> Unit) {
        if (mutable.value.busy) return
        mutable.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            try { action(); success(); refresh() }
            catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { mutable.update { it.copy(error = (failure as? LibraryFailure)?.reason ?: "INTERNAL") } }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }
}
