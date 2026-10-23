package com.grtsinry43.memedock.feature

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.emptyPreferences
import com.grtsinry43.memedock.app.LibraryBinding
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.data.settings.SyncPreferences
import com.grtsinry43.memedock.data.sync.*
import com.grtsinry43.memedock.feature.sync.*
import com.grtsinry43.memedock.platform.credentials.SyncTokenStore
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SyncWorkflowTest {
    private class Store(private val ready: Deferred<Unit>? = null) : DataStore<Preferences> {
        private val values = MutableStateFlow(emptyPreferences())
        override val data = flow { ready?.await(); emitAll(values) }
        override suspend fun updateData(transform: suspend (Preferences) -> Preferences): Preferences =
            transform(values.value).also { values.value = it }
    }
    private class Tokens : SyncTokenStore {
        val values = mutableMapOf<Pair<String, String>, String>()
        var failSave = false
        var failLoad = false
        override suspend fun load(libraryId: String, origin: String): String? {
            if (failLoad) throw java.io.IOException("test key unavailable")
            return values[libraryId to origin]
        }
        override suspend fun save(libraryId: String, origin: String, token: String) {
            if (failSave) throw java.io.IOException("test key unavailable")
            values[libraryId to origin] = token
        }
        override suspend fun clear(libraryId: String, origin: String) { values.remove(libraryId to origin) }
    }
    private class Scheduler : SyncWorkScheduler {
        val configurations = mutableListOf<SyncWorkIdentity?>()
        val requests = mutableListOf<SyncWorkIdentity>()
        val policies = mutableListOf<Boolean>()
        override fun configure(identity: SyncWorkIdentity?, wifiOnly: Boolean) { configurations += identity; policies += wifiOnly }
        override fun request(identity: SyncWorkIdentity, wifiOnly: Boolean) { requests += identity }
        override fun automaticNetworkAvailable(wifiOnly: Boolean) = true
    }
    private class Repository : SyncRepository {
        val current = LibraryBinding(0, "library-a")
        override val bindings = MutableStateFlow<LibraryBinding?>(current)
        val status = MutableStateFlow(SyncStatus(origin = "https://example.test", deviceId = "device-a", connected = true))
        var statusGate: CompletableDeferred<Unit>? = null
        var runGate: CompletableDeferred<Unit>? = null
        var failure: String? = null
        var calls = 0
        var active = 0
        var peak = 0
        var cancellations = 0
        override suspend fun binding() = bindings.filterNotNull().first()
        override suspend fun status(binding: LibraryBinding): SyncStatus { statusGate?.await(); return status.value }
        override fun observe(binding: LibraryBinding): Flow<SyncStatus> = status
        override suspend fun connect(binding: LibraryBinding, origin: String, token: String): SyncStatus =
            status.value.copy(origin = origin, paused = false, error = null).also { status.value = it }
        override suspend fun pause(binding: LibraryBinding, paused: Boolean): SyncStatus =
            status.value.copy(paused = paused).also { status.value = it }
        override suspend fun synchronize(binding: LibraryBinding, token: String): SyncStatus {
            calls++; active++; peak = maxOf(peak, active)
            try {
                runGate?.await()
                failure?.let { status.value = status.value.copy(error = it.lowercase()); throw LibraryFailure(it) }
                return status.value
            } catch (cancel: CancellationException) { cancellations++; throw cancel }
            finally { active-- }
        }
    }
    private class Harness(scope: CoroutineScope, preferencesReady: Deferred<Unit>? = null) {
        val repository = Repository()
        val tokens = Tokens().apply { values["library-a" to "https://example.test"] = "secret" }
        val scheduler = Scheduler()
        val preferences = SyncPreferences(Store(preferencesReady))
        val changes = MutableSharedFlow<LibraryChange>(extraBufferCapacity = 1)
        val model = SyncCoordinator(repository, tokens, scheduler, preferences, changes, scope)
        val identity = SyncWorkIdentity("library-a", "https://example.test", "device-a")
    }
    @Test fun coldStartWaitsForCredentialsWithoutCancellingItsWorker() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            val h = Harness(backgroundScope)
            h.repository.statusGate = CompletableDeferred()
            val worker = async { h.model.background(h.identity) }
            runCurrent()
            assertFalse(worker.isCompleted)
            assertTrue(h.scheduler.configurations.isEmpty())
            h.repository.statusGate!!.complete(Unit); runCurrent()
            assertEquals(SyncRun.Complete, worker.await())
            assertEquals(1, h.repository.calls)
            assertFalse(h.scheduler.configurations.contains(null))
        } finally { Dispatchers.resetMain() }
    }
    @Test fun coldStartCannotSyncBeforeThePersistedUnmeteredPolicyLoads() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            val ready = CompletableDeferred<Unit>()
            val h = Harness(backgroundScope, ready)
            h.preferences.setWifiOnly(true)
            h.model.foreground(true); runCurrent()
            assertTrue(h.scheduler.configurations.isEmpty())
            assertEquals(0, h.repository.calls)
            ready.complete(Unit); runCurrent()
            assertTrue(h.model.state.value.wifiOnly)
            assertTrue(h.scheduler.policies.isNotEmpty())
            assertTrue(h.scheduler.policies.all { it })
        } finally { Dispatchers.resetMain() }
    }
    @Test fun unreadableCredentialsStopAutomaticRequestsUntilReauthorization() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            val h = Harness(backgroundScope); runCurrent()
            h.tokens.failLoad = true
            assertEquals(SyncRun.Blocked, h.model.background(h.identity))
            assertEquals(SyncRun.Skipped, h.model.background(h.identity))
            assertEquals(0, h.repository.calls)
            assertNull(h.scheduler.configurations.last())
        } finally { Dispatchers.resetMain() }
    }
    @Test fun foregroundManualAndWorkerRequestsShareOneLaneAndCancellationDrainsIt() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            val h = Harness(backgroundScope); runCurrent()
            h.repository.runGate = CompletableDeferred()
            val first = launch { h.model.background(h.identity) }; runCurrent()
            h.model.synchronizeNow(); h.model.foreground(true); runCurrent()
            assertEquals(1, h.repository.active)
            first.cancelAndJoin(); runCurrent()
            assertEquals(1, h.repository.peak)
            assertEquals(1, h.repository.cancellations)
            h.repository.runGate!!.complete(Unit); runCurrent()
            assertEquals(0, h.repository.active)
            assertEquals(1, h.repository.peak)
        } finally { Dispatchers.resetMain() }
    }
    @Test fun staleWorkAndRevokedCredentialsCannotIssueFurtherRequests() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            val h = Harness(backgroundScope); runCurrent()
            assertEquals(SyncRun.Skipped, h.model.background(h.identity.copy(deviceId = "old-device")))
            assertEquals(0, h.repository.calls)
            h.repository.failure = "UNAUTHORIZED"
            assertEquals(SyncRun.Blocked, h.model.background(h.identity))
            assertEquals(SyncRun.Skipped, h.model.background(h.identity))
            assertEquals(1, h.repository.calls)
            assertNull(h.scheduler.configurations.last())
        } finally { Dispatchers.resetMain() }
    }
    @Test fun failedCredentialSavePausesBindingAndRestoreDrainsWorkBeforeRebinding() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            val h = Harness(backgroundScope); runCurrent()
            h.tokens.failSave = true
            h.model.connect("https://example.test", "new-secret"); runCurrent()
            assertTrue(h.model.state.value.status.paused)
            assertFalse(h.model.state.value.hasCredential)
            assertEquals("CREDENTIAL", h.model.state.value.error)
            h.tokens.failSave = false
            h.model.connect("https://example.test", "new-secret"); runCurrent()
            h.repository.runGate = CompletableDeferred()
            val worker = launch { h.model.background(h.identity) }; runCurrent()
            h.model.beginMaintenance()
            assertTrue(worker.isCancelled)
            assertEquals(0, h.repository.active)
            assertEquals(SyncRun.Skipped, h.model.background(h.identity))
            h.repository.bindings.value = LibraryBinding(1, "library-b")
            h.repository.status.value = SyncStatus()
            runCurrent(); h.model.endMaintenance()
            assertEquals("library-b", h.model.state.value.binding?.libraryId)
            assertEquals(SyncRun.Skipped, h.model.background(h.identity))
            assertNull(h.scheduler.configurations.last())
        } finally { Dispatchers.resetMain() }
    }
    @Test fun pauseAndDisconnectCancelSchedulingAndLocalWritesRequestDurableWork() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            val h = Harness(backgroundScope); runCurrent()
            h.changes.emit(LibraryChange.Content); runCurrent(); advanceTimeBy(501); runCurrent()
            assertEquals(listOf(h.identity), h.scheduler.requests)
            h.model.setPaused(true); runCurrent()
            assertEquals(SyncRun.Skipped, h.model.background(h.identity))
            h.model.disconnect(); runCurrent()
            assertTrue(h.tokens.values.isEmpty())
            assertFalse(h.model.state.value.hasCredential)
            assertNull(h.scheduler.configurations.last())
        } finally { Dispatchers.resetMain() }
    }
}
