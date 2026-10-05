package com.grtsinry43.memedock.feature

import androidx.lifecycle.ViewModelStore
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.feature.detail.DetailViewModel
import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class DetailWorkflowTest {
    private class Repository : DetailRepository {
        var released = 0
        var recorded = 0
        override suspend fun detail(id: String) = StickerDetails(id, "猫", "", "cat.png", "image/png", 40, 20, 100,
            false, false, emptyList(), emptyList(), "/fixture/cat.png", null)
        override suspend fun exportOriginal(id: String) = object : OutputLease { override fun close() { released++ } }
        override suspend fun prepareHandoff(lease: OutputLease) = ShareArtifact("artifact", "/fixture/share.png", "image/png", "cat.png", 100, false)
        override suspend fun recordShareLaunched(id: String) { recorded++ }
    }
    @Test fun onlyLaunchedSharesAreCountedAndEveryLeaseIsReleased() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            val model = DetailViewModel("id", repository); store.put("detail", model); runCurrent()
            assertEquals("猫", model.state.value.detail?.title)
            model.share { throw SecurityException("grant failed") }; runCurrent()
            assertEquals(0, repository.recorded)
            assertEquals(1, repository.released)
            assertEquals("PERMISSION_DENIED", model.state.value.shareError)
            model.share {}; runCurrent()
            assertEquals(1, repository.recorded)
            assertEquals(2, repository.released)
            assertFalse(model.state.value.sharing)
        } finally { store.clear(); Dispatchers.resetMain() }
    }
    @Test fun editsRejectDoubleSubmissionAndSurfaceFailureBeforeRetry() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val model = DetailViewModel("id", Repository()); store.put("detail", model); runCurrent()
            val gate = CompletableDeferred<Unit>()
            var calls = 0
            var successes = 0
            model.manage({ calls++; gate.await(); throw LibraryFailure("CONFLICT") }, { successes++ })
            model.manage({ calls++ }, { successes++ })
            runCurrent()
            assertTrue(model.state.value.managing)
            assertEquals(1, calls)
            gate.complete(Unit); runCurrent()
            assertFalse(model.state.value.managing)
            assertEquals("CONFLICT", model.state.value.managementError)
            assertEquals(0, successes)
            model.manage({}, { successes++ }); runCurrent()
            assertEquals(1, successes)
            assertNull(model.state.value.managementError)
        } finally { store.clear(); Dispatchers.resetMain() }
    }
}
