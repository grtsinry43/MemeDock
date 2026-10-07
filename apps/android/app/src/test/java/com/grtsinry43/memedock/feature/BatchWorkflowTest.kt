package com.grtsinry43.memedock.feature

import androidx.lifecycle.ViewModelStore
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.feature.library.BatchActionsViewModel
import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class BatchWorkflowTest {
    private class Operation : BatchOperation {
        val result = CompletableDeferred<BatchReport>()
        var cancelled = false
        var closed = false
        override suspend fun awaitResult() = result.await()
        override fun snapshot() = BatchReport(emptyList(), false)
        override fun cancel() { cancelled = true }
        override fun close() { closed = true }
    }
    private class Repository : ManagementRepository {
        val ready = CompletableDeferred<Unit>()
        val operation = Operation()
        var calls = 0
        var received = emptyList<LibraryItem>()
        override suspend fun batch(items: List<LibraryItem>, action: BatchAction): BatchOperation {
            calls++; received = items; ready.await(); return operation
        }
        private fun unexpected(): Nothing = error("Unexpected management call")
        override suspend fun collectionSummaries(): List<CollectionSummary> = unexpected()
        override suspend fun patchSticker(detail: StickerDetails, title: String?, note: String?, starred: Boolean?) = unexpected()
        override suspend fun relations(detail: StickerDetails, collection: LibraryCollection?, tags: List<LibraryTag>?, changeCollection: Boolean) = unexpected()
        override suspend fun createCollection(name: String): LibraryCollection = unexpected()
        override suspend fun renameCollection(value: LibraryCollection, name: String) = unexpected()
        override suspend fun deleteCollection(value: LibraryCollection) = unexpected()
        override suspend fun moveCollection(value: LibraryCollection, before: LibraryCollection?) = unexpected()
        override suspend fun moveCollectionItem(collection: LibraryCollection, value: LibraryItem, before: LibraryItem?) = unexpected()
        override suspend fun collections(deleted: Boolean): List<LibraryCollection> = unexpected()
        override suspend fun tags(deleted: Boolean): List<LibraryTag> = unexpected()
        override suspend fun createTag(name: String): LibraryTag = unexpected()
        override suspend fun renameTag(value: LibraryTag, name: String) = unexpected()
        override suspend fun deleteTag(value: LibraryTag) = unexpected()
        override suspend fun deleteSticker(value: StickerDetails) = unexpected()
        override suspend fun restoreSticker(value: StickerDetails) = unexpected()
        override suspend fun restoreCollection(value: LibraryCollection) = unexpected()
        override suspend fun restoreTag(value: LibraryTag) = unexpected()
        override suspend fun suggestions(id: String): RestoreSuggestions = unexpected()
    }
    private fun item(id: String) = LibraryItem(id, id, "$id.png", false, "image/png", 1, 1,
        ThumbnailState.Missing, null, null)

    @Test fun partialFailureKeepsOnlyRetryableSelectionAndRejectsDuplicateSubmission() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            val model = BatchActionsViewModel(repository); store.put("batch", model)
            model.start(item("a")); model.toggle(item("b"))
            model.run(BatchAction.Star(true)); model.run(BatchAction.Delete); runCurrent()
            model.toggle(item("a")); model.exit()
            assertEquals(setOf("a", "b"), model.state.value.selected.keys)
            assertEquals(1, repository.calls)
            repository.ready.complete(Unit); runCurrent()
            repository.operation.result.complete(BatchReport(listOf(
                BatchItemResult("a", BatchOutcome.Applied, null),
                BatchItemResult("b", BatchOutcome.Failed, "CONFLICT")), false)); runCurrent()
            assertEquals(setOf("b"), model.state.value.selected.keys)
            assertFalse(model.state.value.busy)
            assertTrue(repository.operation.closed)
            model.exit(); assertFalse(model.state.value.selecting)
        } finally { store.clear(); Dispatchers.resetMain() }
    }

    @Test fun stoppingDuringOperationCreationCancelsTheReturnedOperation() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            val model = BatchActionsViewModel(repository); store.put("batch", model)
            model.start(item("a")); model.run(BatchAction.Delete); runCurrent()
            model.stop(); repository.ready.complete(Unit); runCurrent()
            assertTrue(repository.operation.cancelled)
            repository.operation.result.complete(BatchReport(listOf(BatchItemResult("a", BatchOutcome.Pending, null)), true)); runCurrent()
            assertEquals(setOf("a"), model.state.value.selected.keys)
            assertTrue(model.state.value.report?.stopped == true)
        } finally { store.clear(); Dispatchers.resetMain() }
    }
}
