package com.grtsinry43.memedock.feature.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

class LibraryViewModel(private val repository: LibraryRepository, private val collectionId: String? = null) : ViewModel() {
    private val mutable = MutableStateFlow(LibraryUiState())
    val state = mutable.asStateFlow()
    private var cursor: PageCursor? = null
    private var queryJob: Job? = null
    private var generation = 0L
    private val thumbnails = mutableMapOf<String, Job>()
    private var pendingReload = false
    private val pendingThumbnails = mutableSetOf<String>()
    private val refreshes = MutableSharedFlow<Unit>(extraBufferCapacity = 1,
        onBufferOverflow = kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)

    init {
        reload()
        viewModelScope.launch {
            repository.changes.collect { change ->
                when (change) {
                    is LibraryChange.Thumbnail -> if (mutable.value.items.any { it.id == change.id }) pendingThumbnails.add(change.id)
                    else -> pendingReload = true
                }
                refreshes.emit(Unit)
            }
        }
        viewModelScope.launch {
            // Merge bursts without waiting for the final event of a long import.
            refreshes.collect {
                delay(100)
                if (pendingReload) {
                    pendingReload = false
                    pendingThumbnails.clear()
                    reload()
                } else {
                    while (pendingThumbnails.isNotEmpty()) {
                        val ids = pendingThumbnails.take(200)
                        pendingThumbnails.removeAll(ids.toSet())
                        refreshThumbnails(ids)
                    }
                }
            }
        }
    }
    fun search(text: String) {
        if (text == mutable.value.search) return
        mutable.update { it.copy(search = text) }
        reload(debounce = true)
    }
    fun retry() { repository.retryOpen(); reload() }
    fun toggleStarred() { mutable.update { it.copy(starredOnly = !it.starredOnly) }; reload() }
    private fun reload(debounce: Boolean = false) {
        generation++
        val request = generation
        val text = mutable.value.search
        val starred = true.takeIf { mutable.value.starredOnly }
        queryJob?.cancel()
        cursor?.close(); cursor = null
        mutable.update { it.copy(items = emptyList(), loading = true, loadingMore = false, hasMore = false, error = null, pageError = null) }
        queryJob = viewModelScope.launch {
            try {
                if (debounce) delay(250)
                val page = repository.page(text, collectionId = collectionId, starred = starred)
                if (request != generation) { page.next?.close(); return@launch }
                cursor = page.next
                mutable.update { it.copy(items = page.items, loading = false, hasMore = page.next != null) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { if (request == generation) mutable.update { it.copy(loading = false, error = reason(error)) } }
        }
    }
    fun loadMore() {
        val next = cursor ?: return
        if (mutable.value.loading || mutable.value.loadingMore) return
        val request = generation
        val text = mutable.value.search
        mutable.update { it.copy(loadingMore = true, pageError = null) }
        queryJob = viewModelScope.launch {
            try {
                val page = repository.page(text, next, collectionId, starred = true.takeIf { mutable.value.starredOnly })
                if (request != generation) { page.next?.close(); return@launch }
                next.close(); cursor = page.next
                mutable.update { it.copy(items = (it.items + page.items).distinctBy(LibraryItem::id), loadingMore = false, hasMore = page.next != null) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { if (request == generation) mutable.update { it.copy(loadingMore = false, pageError = reason(error)) } }
        }
    }
    fun ensureThumbnail(item: LibraryItem, retry: Boolean = false) {
        if ((!retry && item.thumbnailPath != null) || thumbnails[item.id]?.isActive == true || (!retry && item.thumbnailState == ThumbnailState.Failed)) return
        // Native admission and image budgets bound work; do not queue the whole library.
        if (thumbnails.values.count { it.isActive } >= 8) return
        val job = viewModelScope.launch(start = CoroutineStart.LAZY) {
            try {
                val path = repository.thumbnail(item.id)
                mutable.update { state -> state.copy(items = state.items.map { if (it.id == item.id) it.copy(thumbnailPath = path, thumbnailState = ThumbnailState.Ready, error = null) else it }) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) {
                val code = reason(error)
                if (code == "BUSY") delay(250)
                if (code != "BUSY") mutable.update { state -> state.copy(items = state.items.map { if (it.id == item.id) it.copy(thumbnailState = ThumbnailState.Failed, error = code) else it }) }
            } finally {
                thumbnails.remove(item.id)
                mutable.update { it.copy(thumbnailEpoch = it.thumbnailEpoch + 1) }
            }
        }
        thumbnails[item.id] = job
        job.start()
    }
    private suspend fun refreshThumbnails(ids: List<String>) {
        try {
            val updates = repository.thumbnailStates(ids).associateBy(ThumbnailUpdate::id)
            mutable.update { state -> state.copy(items = state.items.map { item ->
                updates[item.id]?.let { update -> item.copy(thumbnailState = update.state, thumbnailPath = update.path, error = update.error) } ?: item
            }) }
        } catch (cancel: CancellationException) { throw cancel }
        catch (error: Exception) { mutable.update { it.copy(pageError = reason(error)) } }
    }
    private fun reason(error: Exception) = (error as? LibraryFailure)?.reason ?: "INTERNAL"
    override fun onCleared() { cursor?.close(); super.onCleared() }
}
