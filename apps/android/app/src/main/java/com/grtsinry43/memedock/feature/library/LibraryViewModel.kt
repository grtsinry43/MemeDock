package com.grtsinry43.memedock.feature.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Grid state for the home page, for one collection or tag when [collectionId] or [tagId] is set, or for the
 * trash when [deleted] is set. Home also lists [collections] so they can be offered as filters.
 */
class LibraryViewModel(
    private val repository: LibraryRepository,
    private val collectionId: String? = null,
    private val collections: CollectionRepository? = null,
    private val tagId: String? = null,
    private val deleted: Boolean = false,
) : ViewModel() {
    private data class Query(val text: String, val collectionId: String?, val starred: Boolean?, val sort: LibrarySort?,
        val tagIds: List<String>, val deleted: Boolean)

    private val mutable = MutableStateFlow(LibraryUiState())
    val state = mutable.asStateFlow()
    private val failureChannel = Channel<String>(Channel.BUFFERED)
    /** One-shot failures of actions started from the grid, such as a move that did not save. */
    val failures = failureChannel.receiveAsFlow()
    private val moves = Mutex()
    private val scoped = collectionId != null || tagId != null || deleted
    private var cursor: PageCursor? = null
    private var active: Query? = null
    private var queryJob: Job? = null
    private var collectionsJob: Job? = null
    private var generation = 0L
    private val thumbnails = mutableMapOf<String, Job>()
    private var pendingReload = false
    private val pendingThumbnails = mutableSetOf<String>()
    private val refreshes = MutableSharedFlow<Unit>(extraBufferCapacity = 1,
        onBufferOverflow = kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)

    init {
        reload()
        loadCollections()
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
                    reload(keepItems = true)
                    loadCollections()
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

    fun select(filter: LibraryFilter) {
        if (filter == mutable.value.filter) return
        mutable.update { it.copy(filter = filter) }
        reload()
    }

    fun retry() { repository.retryOpen(); reload(); loadCollections() }

    private fun query(state: LibraryUiState): Query {
        val filter = if (scoped) LibraryFilter.All else state.filter
        val collection = collectionId ?: (filter as? LibraryFilter.Collection)?.id
        return Query(
            text = state.search,
            collectionId = collection,
            starred = true.takeIf { filter == LibraryFilter.Starred },
            sort = when (filter) {
                LibraryFilter.Recent -> LibrarySort.LastUsed
                LibraryFilter.All, LibraryFilter.Starred -> LibrarySort.Added
                is LibraryFilter.Collection -> null
            }.takeIf { !scoped },
            tagIds = listOfNotNull(tagId),
            deleted = deleted,
        )
    }

    /**
     * Shows [order] at once and saves where [moved] landed through [commit], which receives the item now
     * following it. [order] must hold every page, otherwise the last loaded item has an unknown successor.
     * Moves save one at a time; a failure reloads the saved order.
     */
    fun reorder(order: List<LibraryItem>, moved: LibraryItem, commit: suspend (before: LibraryItem?) -> Unit) {
        val index = order.indexOfFirst { it.id == moved.id }
        if (index < 0 || cursor != null) return
        val before = order.getOrNull(index + 1)
        mutable.update { it.copy(items = order) }
        viewModelScope.launch {
            moves.withLock {
                try { commit(before) }
                catch (cancel: CancellationException) { throw cancel }
                catch (error: Exception) {
                    failureChannel.send(reason(error))
                    reload(keepItems = true)
                }
            }
        }
    }

    /** [keepItems] refreshes in place so library changes do not flash the grid back to placeholders. */
    private fun reload(debounce: Boolean = false, keepItems: Boolean = false) {
        generation++
        val request = generation
        val query = query(mutable.value)
        queryJob?.cancel()
        cursor?.close(); cursor = null; active = null
        mutable.update {
            if (keepItems) it.copy(loadingMore = false, pageError = null)
            else it.copy(items = emptyList(), loading = true, loadingMore = false, hasMore = false, error = null, pageError = null)
        }
        queryJob = viewModelScope.launch {
            try {
                if (debounce) delay(250)
                val page = fetch(query, null)
                if (request != generation) { page.next?.close(); return@launch }
                cursor = page.next; active = query
                mutable.update { it.copy(items = page.items, loading = false, error = null, hasMore = page.next != null) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) {
                if (request == generation) mutable.update {
                    if (keepItems && it.items.isNotEmpty()) it.copy(pageError = reason(error)) else it.copy(loading = false, error = reason(error))
                }
            }
        }
    }

    fun loadMore() {
        val next = cursor ?: return
        val query = active ?: return
        if (mutable.value.loading || mutable.value.loadingMore) return
        val request = generation
        mutable.update { it.copy(loadingMore = true, pageError = null) }
        queryJob = viewModelScope.launch {
            try {
                val page = fetch(query, next)
                if (request != generation) { page.next?.close(); return@launch }
                next.close(); cursor = page.next
                mutable.update { it.copy(items = (it.items + page.items).distinctBy(LibraryItem::id), loadingMore = false, hasMore = page.next != null) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { if (request == generation) mutable.update { it.copy(loadingMore = false, pageError = reason(error)) } }
        }
    }

    private suspend fun fetch(query: Query, next: PageCursor?) =
        repository.page(query.text, next, query.collectionId, query.starred, query.deleted, query.tagIds, query.sort)

    private fun loadCollections() {
        val source = collections ?: return
        collectionsJob?.cancel()
        collectionsJob = viewModelScope.launch {
            try {
                val values = source.collections()
                val filter = mutable.value.filter
                val gone = filter is LibraryFilter.Collection && values.none { it.id == filter.id }
                mutable.update { it.copy(collections = values) }
                if (gone) select(LibraryFilter.Recent)
            } catch (cancel: CancellationException) { throw cancel }
            catch (_: Exception) { /* Filters are optional; the grid reports library failures. */ }
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
