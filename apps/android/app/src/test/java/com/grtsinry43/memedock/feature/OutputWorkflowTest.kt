package com.grtsinry43.memedock.feature

import androidx.lifecycle.ViewModelStore
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.feature.detail.DetailViewModel
import com.grtsinry43.memedock.platform.clipboard.*
import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class OutputWorkflowTest {
    private class Repository : DetailRepository, ClipboardRepository {
        val events = mutableListOf<String>()
        var choice: ExportChoice? = null
        var consent = false
        var block: CompletableDeferred<Unit>? = null
        override suspend fun detail(id: String) = StickerDetails(id, "猫", "", "cat.webp", "image/webp", 40, 20, 100,
            true, false, emptyList(), null, "/fixture/cat.webp", null)
        override suspend fun exportOriginal(id: String) = export(id, ExportChoice.Original, false)
        override suspend fun export(id: String, choice: ExportChoice, firstFrame: Boolean): OutputLease {
            this.choice = choice; consent = firstFrame; block?.await()
            events += "export"
            return object : OutputLease { override fun close() { events += "close" } }
        }
        override suspend fun prepareHandoff(lease: OutputLease): ShareArtifact {
            events += "verify"; return ShareArtifact("output", "/fixture/output.png", "image/png", "cat.png", 100, false)
        }
        override suspend fun recordShareLaunched(id: String) { events += "share-used" }
        override suspend fun recordCopy(id: String) { events += "copy-used" }
        override suspend fun recordSaved(id: String) { events += "save-used" }
        override suspend fun protectClipboard(lease: OutputLease): String { events += "protect"; return "pin" }
        override suspend fun reconcileClipboard(observed: String?) { events += "confirm:$observed" }
        override suspend fun abortClipboard(reference: String) { events += "abort:$reference" }
    }
    @Test fun copyingProtectsBeforeHandoffConfirmsAfterAndRecordsOnlySuccess() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            var fail = true
            val gateway = object : ClipboardGateway {
                override fun copy(artifact: ShareArtifact, reference: String) {
                    repository.events += "copy"
                    if (fail) throw SecurityException("Denied")
                }
                override fun observedReference(): String? = "pin"
            }
            val coordinator = ClipboardCoordinator(repository, gateway, backgroundScope)
            val model = DetailViewModel("id", repository); store.put("detail", model); runCurrent()
            model.copy(ExportChoice.CompatiblePng, true, coordinator); runCurrent()
            assertEquals(listOf("export", "verify", "protect", "copy", "abort:pin", "close"), repository.events)
            assertEquals("PERMISSION_DENIED", model.state.value.shareError)
            repository.events.clear(); fail = false
            model.copy(ExportChoice.WhiteBackground, true, coordinator); runCurrent()
            assertEquals(listOf("export", "verify", "protect", "copy", "confirm:pin", "copy-used", "close"), repository.events)
            assertEquals(ExportChoice.WhiteBackground, repository.choice)
            assertTrue(repository.consent); assertTrue(model.state.value.copied)
        } finally { store.clear(); Dispatchers.resetMain() }
    }
    @Test fun cancellingPreparationNeverHandsOffOrCountsUsage() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository().apply { block = CompletableDeferred() }
            val model = DetailViewModel("id", repository); store.put("detail", model); runCurrent()
            model.share(ExportChoice.SmallJpeg, true) { fail("Cancelled handoff") }; runCurrent()
            model.cancelShare(); runCurrent()
            assertFalse(model.state.value.sharing); assertNull(model.state.value.shareError)
            assertTrue(repository.events.isEmpty())
        } finally { store.clear(); Dispatchers.resetMain() }
    }
}
