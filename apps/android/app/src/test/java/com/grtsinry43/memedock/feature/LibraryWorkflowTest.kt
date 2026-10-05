package com.grtsinry43.memedock.feature

import androidx.lifecycle.ViewModelStore
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.feature.importing.*
import com.grtsinry43.memedock.feature.library.LibraryViewModel
import com.grtsinry43.memedock.platform.importing.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class LibraryWorkflowTest {
    private class Slot : ImportSlot {
        override val path = "test-only-slot"
        var closed = false
        override fun close() { closed = true }
    }
    private class Cursor : PageCursor {
        var closed = false
        override fun close() { closed = true }
    }
    private class Repository : LibraryRepository {
        override val changes = MutableSharedFlow<LibraryChange>()
        var pages: suspend (String, PageCursor?) -> LibraryPage = { _, _ -> LibraryPage(emptyList(), null) }
        val slots = mutableListOf<Slot>()
        val imported = mutableListOf<String>()
        var discarded = 0
        override suspend fun page(text: String, cursor: PageCursor?) = pages(text, cursor)
        override suspend fun thumbnail(id: String) = "/test/$id.png"
        override suspend fun thumbnailStates(ids: List<String>) = ids.map { ThumbnailUpdate(it, ThumbnailState.Ready, "/test/$it.png", null) }
        override suspend fun createInput() = Slot().also { slots.add(it) }
        override suspend fun discardInput(input: ImportSlot) { discarded++ }
        override suspend fun importInput(input: ImportSlot, name: String, cancelled: () -> Boolean): ImportedItem {
            if (cancelled()) throw LibraryFailure("CANCELLED")
            imported.add(name)
            return ImportedItem(name, if (name == "duplicate") ImportDisposition.Reused else ImportDisposition.Created)
        }
        override fun retryOpen() = Unit
    }
    private class Gateway : ImportGateway {
        var onCopy: suspend (ImportCandidate) -> Unit = { if (it.name == "bad") throw java.io.IOException("provider failed") }
        override suspend fun describe(uri: String) = ImportCandidate(uri, uri, null)
        override suspend fun copy(candidate: ImportCandidate, destination: String, cancelled: () -> Boolean, progress: (Long) -> Unit) {
            onCopy(candidate)
            if (cancelled()) throw LibraryFailure("CANCELLED")
            progress(512)
        }
        override suspend fun cancelActiveRead() = Unit
    }
    private fun item(id: String) = LibraryItem(id, id, "$id.png", false, "image/png", 40, 20, ThumbnailState.Missing, null, null)

    @Test fun batchKeepsPartialSuccessAndClosesEverySlot() = runTest {
        val repository = Repository()
        val coordinator = ImportCoordinator(repository, Gateway(), backgroundScope)
        coordinator.prepare(listOf("good", "bad", "duplicate")); runCurrent()
        coordinator.start(); runCurrent()
        assertEquals(ImportPhase.Finished, coordinator.state.value.phase)
        assertEquals(listOf(ImportItemStatus.Created, ImportItemStatus.Failed, ImportItemStatus.Reused), coordinator.state.value.items.map { it.status })
        assertEquals(listOf("good", "duplicate"), repository.imported)
        assertEquals(3, repository.discarded)
        assertTrue(repository.slots.all { it.closed })
        coordinator.hide(); coordinator.show()
        assertEquals(3, coordinator.state.value.items.size)
    }

    @Test fun cancellationStopsRemainingItemsAndRetryDoesNotRepeatSuccess() = runTest {
        val repository = Repository()
        val gateway = Gateway()
        val gate = CompletableDeferred<Unit>()
        gateway.onCopy = { if (it.name == "slow") gate.await() }
        val coordinator = ImportCoordinator(repository, gateway, backgroundScope)
        coordinator.prepare(listOf("good", "slow", "later")); runCurrent()
        coordinator.start(); runCurrent()
        coordinator.cancel(); gate.complete(Unit); runCurrent()
        assertEquals(listOf(ImportItemStatus.Created, ImportItemStatus.Cancelled, ImportItemStatus.Cancelled), coordinator.state.value.items.map { it.status })
        coordinator.retryFailed(); runCurrent()
        assertEquals(listOf("good", "slow", "later"), repository.imported)
        assertTrue(repository.slots.all { it.closed })
    }

    @Test fun oversizedBatchIsRejectedWithoutReadingUris() = runTest {
        val repository = Repository()
        val coordinator = ImportCoordinator(repository, Gateway(), backgroundScope)
        coordinator.prepare((0..200).map { it.toString() }); runCurrent()
        assertEquals("BATCH_LIMIT", coordinator.state.value.selectionError)
        assertTrue(repository.slots.isEmpty())
    }

    @Test fun debouncedSearchDropsOldResultsAndClosesCursors() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            val cursor = Cursor()
            repository.pages = { text, _ -> if (text.isEmpty()) LibraryPage(listOf(item("initial")), cursor) else LibraryPage(listOf(item(text)), null) }
            val model = LibraryViewModel(repository); store.put("library", model); runCurrent()
            model.search("old"); model.search("new"); advanceTimeBy(251); runCurrent()
            assertEquals(listOf("new"), model.state.value.items.map { it.id })
            assertTrue(cursor.closed)
            assertFalse(model.state.value.loading)
        } finally { store.clear(); Dispatchers.resetMain() }
    }

    @Test fun paginationAppendsAndThumbnailEventsPreserveLoadedPages() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            val cursor = Cursor()
            var queries = 0
            repository.pages = { _, next -> queries++; if (next == null) LibraryPage(listOf(item("first")), cursor) else LibraryPage(listOf(item("second")), null) }
            val model = LibraryViewModel(repository); store.put("library", model); runCurrent()
            model.loadMore(); runCurrent()
            repository.changes.emit(LibraryChange.Thumbnail("first")); advanceTimeBy(101); runCurrent()
            assertEquals(listOf("first", "second"), model.state.value.items.map { it.id })
            assertEquals("/test/first.png", model.state.value.items.first().thumbnailPath)
            assertEquals(2, queries)
            assertTrue(cursor.closed)
        } finally { store.clear(); Dispatchers.resetMain() }
    }
}
