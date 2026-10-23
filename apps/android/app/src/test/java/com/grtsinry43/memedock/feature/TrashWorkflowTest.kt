package com.grtsinry43.memedock.feature

import com.grtsinry43.memedock.app.LibraryBinding
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.feature.trash.TrashCoordinator
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class TrashWorkflowTest {
    private class Repository : TrashRepository {
        override val bindings = MutableStateFlow<LibraryBinding?>(LibraryBinding(0, "a"))
        var entries = listOf(TrashEntry(TrashKind.Sticker, "image", 2, 19),
            TrashEntry(TrashKind.Collection, "collection", 1, 12), TrashEntry(TrashKind.Tag, "tag", 3, 20))
        val purged = mutableListOf<TrashEntry>()
        var fail: String? = null
        var gate: CompletableDeferred<Unit>? = null
        var cleaned = false
        override suspend fun binding() = bindings.filterNotNull().first()
        override suspend fun snapshot(binding: LibraryBinding) = entries.toList()
        override suspend fun purge(binding: LibraryBinding, entry: TrashEntry) {
            gate?.await()
            if (entry.id == fail) throw LibraryFailure("CONFLICT")
            purged += entry
        }
        override suspend fun cleanup(binding: LibraryBinding) { cleaned = true }
    }
    @Test fun emptyTrashUsesAllKindsAndKeepsItsConfirmationSnapshot() = runTest {
        val repository = Repository()
        val model = TrashCoordinator(repository, backgroundScope)
        runCurrent(); model.empty(); runCurrent()
        val original = repository.entries
        repository.entries += TrashEntry(TrashKind.Sticker, "later", 0, 1)
        model.confirm(); model.confirm(); runCurrent()
        assertEquals(original, repository.purged)
        assertEquals(3, model.state.value.report?.succeeded?.size)
        assertEquals(0, model.state.value.report?.failed)
        assertNull(model.state.value.request)
    }
    @Test fun partialFailureKeepsExactReferencesAndReportsSuccessfulSelectionOnly() = runTest {
        val repository = Repository(); repository.fail = "collection"
        val model = TrashCoordinator(repository, backgroundScope)
        var completed = emptySet<TrashEntry>()
        runCurrent(); model.request(repository.entries) { completed = it }; runCurrent()
        model.confirm(); runCurrent()
        assertEquals(setOf(repository.entries[0], repository.entries[2]), completed)
        assertEquals(1, model.state.value.report?.failed)
        assertEquals("CONFLICT", model.state.value.report?.error)
        assertEquals(2L, repository.purged.first().generation)
        assertEquals(19L, repository.purged.first().deletedRevision)
        model.retryCleanup(); runCurrent()
        assertTrue(repository.cleaned)
        assertTrue(model.state.value.cleanupDone)
    }
    @Test fun switchingLibraryInvalidatesConfirmationBeforeAnyDeletion() = runTest {
        val repository = Repository()
        val model = TrashCoordinator(repository, backgroundScope)
        runCurrent(); model.empty(); runCurrent()
        repository.bindings.value = LibraryBinding(1, "b"); runCurrent()
        model.confirm(); runCurrent()
        assertTrue(repository.purged.isEmpty())
        assertNull(model.state.value.request)
        assertEquals("CONFLICT", model.state.value.error)
    }
    @Test fun restoreCancelsAndDrainsDeletionAndClearsPendingConfirmation() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            val repository = Repository(); repository.gate = CompletableDeferred()
            val model = TrashCoordinator(repository, backgroundScope)
            runCurrent(); model.empty(); runCurrent(); model.confirm(); runCurrent()
            model.beginMaintenance()
            repository.gate!!.complete(Unit); runCurrent()
            assertTrue(repository.purged.isEmpty())
            assertFalse(model.state.value.busy)
            assertNull(model.state.value.request)
            model.empty(); runCurrent(); assertNull(model.state.value.request)
            model.endMaintenance(); model.empty(); runCurrent()
            assertNotNull(model.state.value.request)
        } finally { Dispatchers.resetMain() }
    }
}
