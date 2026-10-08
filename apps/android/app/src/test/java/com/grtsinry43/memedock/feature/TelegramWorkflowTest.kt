package com.grtsinry43.memedock.feature

import androidx.lifecycle.ViewModelStore
import com.grtsinry43.memedock.data.telegram.*
import com.grtsinry43.memedock.feature.telegram.TelegramImportViewModel
import com.grtsinry43.memedock.platform.credentials.TelegramTokenStore
import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class TelegramWorkflowTest {
    private class Tokens : TelegramTokenStore {
        var saved: String? = null
        override suspend fun load() = saved
        override suspend fun save(token: String) { saved = token }
        override suspend fun clear() { saved = null }
    }
    private class Pack : TelegramPack {
        override val title = "Cats"
        override val items = listOf(
            TelegramItem("new", "🐈", true, TelegramItemState.Available, true),
            TelegramItem("bad", null, false, TelegramItemState.Available, true),
            TelegramItem("existing", null, false, TelegramItemState.Imported, true),
            TelegramItem("deleted", null, false, TelegramItemState.RestoreRequired, true),
        )
        var closed = false
        override fun close() { closed = true }
    }
    private class Repository : TelegramRepository {
        val pack = Pack()
        var loads = 0
        var imports = 0
        var imported = emptyList<String>()
        var loadGate: CompletableDeferred<Unit>? = null
        var importGate: CompletableDeferred<Unit>? = null
        var previewGate: CompletableDeferred<Unit>? = null
        var previewActive = 0
        var previewPeak = 0
        var loadCancelled = false
        override suspend fun load(token: String, name: String): TelegramPack {
            loads++
            try { loadGate?.await() } catch (cancel: CancellationException) { loadCancelled = true; throw cancel }
            return pack
        }
        override suspend fun preview(pack: TelegramPack, id: String): String {
            previewActive++; previewPeak = maxOf(previewPeak, previewActive)
            try { previewGate?.await(); return "/preview/$id.png" } finally { previewActive-- }
        }
        override suspend fun import(pack: TelegramPack, ids: List<String>, cancelled: () -> Boolean,
            progress: (TelegramReport) -> Unit): TelegramReport {
            imports++; imported = ids
            progress(TelegramReport(ids.map { TelegramResult(it, if (it == "new") TelegramOutcome.Created else TelegramOutcome.Pending) }, false))
            importGate?.await()
            return TelegramReport(ids.map { TelegramResult(it, when {
                it == "new" -> TelegramOutcome.Created
                cancelled() -> TelegramOutcome.Pending
                else -> TelegramOutcome.Failed
            }, if (it == "bad" && !cancelled()) "INVALID_IMAGE" else null) }, cancelled())
        }
    }
    @Test fun explicitSelectionPartialFailureAndRetryPreserveSuccessfulItems() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            val tokens = Tokens()
            val model = TelegramImportViewModel(repository, tokens); store.put("telegram", model); runCurrent()
            model.token("secret-token"); model.name("cats"); model.remember(true)
            assertFalse(model.state.value.toString().contains("secret-token"))
            repository.loadGate = CompletableDeferred()
            model.load(); model.load(); runCurrent()
            assertEquals(1, repository.loads)
            repository.loadGate!!.complete(Unit); runCurrent()
            assertEquals("secret-token", tokens.saved)
            assertTrue(model.state.value.selected.isEmpty())
            model.toggle("deleted"); assertTrue(model.state.value.selected.isEmpty())
            model.selectAll(); assertEquals(setOf("new", "bad"), model.state.value.selected)
            repository.importGate = CompletableDeferred()
            model.importSelected(); model.importSelected(); runCurrent()
            assertEquals(1, repository.imports)
            assertEquals(setOf("new", "bad"), repository.imported.toSet())
            model.toggle("existing"); assertFalse("existing" in model.state.value.selected)
            repository.importGate!!.complete(Unit); runCurrent()
            assertEquals(setOf("bad"), model.state.value.selected)
            assertEquals(TelegramItemState.Imported, model.state.value.itemStates["new"])
            model.selectAll(); assertEquals(setOf("bad"), model.state.value.selected)
            model.changePack(); assertTrue(repository.pack.closed)
        } finally { store.clear(); Dispatchers.resetMain() }
    }
    @Test fun cooperativeCancellationKeepsCompletedItemsAndReleasesPackOnExit() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            val model = TelegramImportViewModel(repository, Tokens()); store.put("telegram", model); runCurrent()
            model.token("token"); model.name("cats"); model.load(); runCurrent()
            model.selectAll(); repository.importGate = CompletableDeferred()
            model.importSelected(); runCurrent(); model.cancel()
            assertTrue(model.state.value.stopping)
            repository.importGate!!.complete(Unit); runCurrent()
            assertTrue(model.state.value.report!!.stopped)
            assertEquals(setOf("bad"), model.state.value.selected)
            assertEquals(TelegramItemState.Imported, model.state.value.itemStates["new"])
            assertFalse(model.state.value.importing)
            store.clear(); assertTrue(repository.pack.closed)
        } finally { store.clear(); Dispatchers.resetMain() }
    }
    @Test fun cancellingLoadsAndPreviewsDoesNotLeakConcurrencyOrSaveCredentials() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        val store = ViewModelStore()
        try {
            val repository = Repository()
            val tokens = Tokens()
            val model = TelegramImportViewModel(repository, tokens); store.put("telegram", model); runCurrent()
            model.token("token"); model.name("cats"); model.remember(true)
            repository.loadGate = CompletableDeferred()
            model.load(); runCurrent(); model.cancel(); runCurrent()
            assertTrue(repository.loadCancelled)
            assertNull(tokens.saved)
            assertFalse(model.state.value.loading)
            repository.previewGate = CompletableDeferred()
            val requests = (1..6).map { launch { model.preview(repository.pack, it.toString()) } }
            runCurrent(); assertEquals(2, repository.previewActive)
            requests.first().cancelAndJoin(); runCurrent()
            assertEquals(2, repository.previewActive)
            repository.previewGate!!.complete(Unit); requests.forEach { it.join() }
            assertEquals(2, repository.previewPeak)
            assertEquals(0, repository.previewActive)
            tokens.saved = "persisted"
            model.remember(false); runCurrent()
            assertNull(tokens.saved)
            assertEquals("token", model.state.value.token)
            model.forget(); runCurrent(); assertEquals("", model.state.value.token)
        } finally { store.clear(); Dispatchers.resetMain() }
    }
}
