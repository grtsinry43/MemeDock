package com.grtsinry43.memedock.feature.sync

import com.grtsinry43.memedock.app.LibraryBinding
import com.grtsinry43.memedock.data.library.LibraryChange
import com.grtsinry43.memedock.data.settings.SyncPreferences
import com.grtsinry43.memedock.data.sync.*
import com.grtsinry43.memedock.platform.credentials.SyncTokenStore
import com.grtsinry43.memedock.ui.failureCode
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

data class SyncState(
    val binding: LibraryBinding? = null, val status: SyncStatus = SyncStatus(),
    val running: Boolean = false, val hasCredential: Boolean = false,
    val wifiOnly: Boolean = false, val error: String? = null, val loaded: Boolean = false,
    val preferencesLoaded: Boolean = false,
    val changing: Boolean = false,
) { val busy: Boolean get() = running || changing }
enum class SyncRun { Complete, Skipped, Retry, Blocked }

/** Owns one serial execution lane shared by the UI and all WorkManager workers. */
@OptIn(FlowPreview::class)
class SyncCoordinator(
    private val repository: SyncRepository, private val tokens: SyncTokenStore,
    private val scheduler: SyncWorkScheduler, private val preferences: SyncPreferences,
    changes: Flow<LibraryChange>, private val scope: CoroutineScope,
) {
    private val mutable = MutableStateFlow(SyncState())
    val state = mutable.asStateFlow()
    private val gate = Mutex()
    private val wake = Channel<Unit>(Channel.CONFLATED)
    private var loop: Job? = null
    private var active: Job? = null
    private var foreground = false
    private var maintenance = false
    private var command: Job? = null

    init {
        scope.launch {
            repository.bindings.collectLatest { binding ->
                stopLoop()
                val wasLoaded = mutable.value.loaded
                mutable.value = SyncState(binding = binding, wifiOnly = mutable.value.wifiOnly,
                    preferencesLoaded = mutable.value.preferencesLoaded)
                if (binding == null) {
                    if (wasLoaded) scheduler.configure(null, mutable.value.wifiOnly)
                    return@collectLatest
                }
                try {
                    gate.withLock {
                        val status = repository.status(binding)
                        mutable.update { it.copy(status = status) }
                        val credential = status.origin?.let { loadToken(binding, it) } != null
                        mutable.update { it.copy(status = status, hasCredential = credential, loaded = true) }
                        schedule()
                    }
                    startLoop()
                    repository.observe(binding).collect { status ->
                        gate.withLock {
                            if (mutable.value.binding == binding) {
                                mutable.update { it.copy(status = status) }
                                schedule()
                            }
                        }
                    }
                } catch (cancel: CancellationException) { throw cancel }
                catch (error: Exception) { failed(error); scheduler.configure(null, mutable.value.wifiOnly) }
            }
        }
        scope.launch {
            try {
                preferences.wifiOnly.collect { wifi ->
                    gate.withLock { mutable.update { it.copy(wifiOnly = wifi, preferencesLoaded = true) }; schedule() }
                    wake.trySend(Unit)
                }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { failed(error) }
        }
        scope.launch {
            changes.filter { it !is LibraryChange.Thumbnail }.debounce(500).collect {
                gate.withLock {
                    val current = mutable.value
                    if (enabled()) current.binding?.let { current.status.workIdentity(it) }?.let {
                        scheduler.request(it, current.wifiOnly)
                    }
                }
                wake.trySend(Unit)
            }
        }
    }

    fun foreground(value: Boolean) {
        foreground = value
        if (value) startLoop() else { loop?.cancel(); loop = null }
    }
    private fun enabled(): Boolean = !maintenance && mutable.value.let {
        it.binding != null && it.preferencesLoaded && it.status.connected && !it.status.paused && it.hasCredential &&
            !it.status.error.equals("unauthorized", ignoreCase = true) &&
            it.error?.uppercase() !in setOf("UNAUTHORIZED", "CREDENTIAL", "INVALID_INPUT", "UNSUPPORTED_SCHEMA", "CORRUPT_DATA")
    }
    private fun schedule() {
        val current = mutable.value
        if (!current.loaded || !current.preferencesLoaded) return
        scheduler.configure(if (enabled()) current.binding?.let { current.status.workIdentity(it) } else null, current.wifiOnly)
    }
    private fun startLoop() {
        if (!foreground || maintenance || loop?.isActive == true) return
        loop = scope.launch {
            while (isActive && foreground && !maintenance) {
                if (enabled() && scheduler.automaticNetworkAvailable(mutable.value.wifiOnly)) synchronize()
                val delayMs = mutable.value.status.retryAfterMs.coerceIn(500, 60_000)
                withTimeoutOrNull(delayMs) { wake.receive() }
            }
        }
    }
    private suspend fun stopLoop() { val previous = loop; loop = null; previous?.cancelAndJoin() }

    private fun launchCommand(action: suspend () -> Unit) {
        if (maintenance || command?.isActive == true) return
        command = scope.launch {
            mutable.update { it.copy(changing = true) }
            try { action() }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { failed(error) }
            finally { mutable.update { it.copy(changing = false) }; startLoop() }
        }
    }
    fun connect(origin: String, token: String) = launchCommand {
        stopLoop()
        gate.withLock {
            if (maintenance) return@withLock
            mutable.update { it.copy(running = true, error = null) }
            try {
                val binding = repository.binding()
                val status = repository.connect(binding, origin.trim(), token)
                val normalized = status.origin ?: error("Missing server origin")
                try { tokens.save(binding.libraryId, normalized, token) }
                catch (cancel: CancellationException) { throw cancel }
                catch (error: Exception) {
                    val paused = repository.pause(binding, true)
                    mutable.update { it.copy(binding = binding, status = paused, hasCredential = false, loaded = true) }
                    throw com.grtsinry43.memedock.data.library.LibraryFailure("CREDENTIAL")
                }
                mutable.update { it.copy(binding = binding, status = status, hasCredential = true, loaded = true) }
            } finally { mutable.update { it.copy(running = false) }; schedule() }
        }
        wake.trySend(Unit)
    }
    fun synchronizeNow() = launchCommand { synchronize(manual = true) }
    fun setPaused(paused: Boolean) = launchCommand {
        stopLoop()
        gate.withLock {
            if (maintenance) return@withLock
            val binding = mutable.value.binding ?: return@withLock
            val status = repository.pause(binding, paused)
            mutable.update { it.copy(status = status, error = null) }
            schedule()
        }
        wake.trySend(Unit)
    }
    fun disconnect() = launchCommand {
        stopLoop()
        gate.withLock {
            val binding = mutable.value.binding ?: return@withLock
            val status = repository.pause(binding, true)
            mutable.update { it.copy(status = status) }
            schedule()
            status.origin?.let { tokens.clear(binding.libraryId, it) }
            mutable.update { it.copy(hasCredential = false, error = null) }
        }
    }
    fun setWifiOnly(value: Boolean) = launchCommand { preferences.setWifiOnly(value) }

    suspend fun background(identity: SyncWorkIdentity): SyncRun = withContext(Dispatchers.Main.immediate) {
        // A cold start must wait for the unique library to open and for its credentials to load.
        val binding = repository.binding()
        state.first { it.binding == binding && ((it.loaded && it.preferencesLoaded) || it.error != null) }
        gate.withLock { /* Initial status/credential loading owns the same lane. */ }
        synchronize(expected = identity)
    }
    private suspend fun synchronize(expected: SyncWorkIdentity? = null, manual: Boolean = false): SyncRun = gate.withLock {
        val current = mutable.value
        val binding = current.binding ?: return@withLock SyncRun.Skipped
        if (!enabled() || (expected != null && current.status.workIdentity(binding) != expected)) return@withLock SyncRun.Skipped
        if (!manual && !scheduler.automaticNetworkAvailable(current.wifiOnly)) return@withLock SyncRun.Retry
        val origin = current.status.origin ?: return@withLock SyncRun.Skipped
        active = currentCoroutineContext().job
        mutable.update { it.copy(running = true, error = null) }
        try {
            val token = loadToken(binding, origin)
            if (token == null) {
                mutable.update { it.copy(hasCredential = false) }
                return@withLock SyncRun.Skipped
            }
            val status = repository.synchronize(binding, token)
            mutable.update { it.copy(status = status) }
            if (status.pending > 0 && status.blocked == 0L) SyncRun.Retry else SyncRun.Complete
        } catch (cancel: CancellationException) { throw cancel }
        catch (error: Exception) {
            failed(error)
            try { val status = repository.status(binding); mutable.update { it.copy(status = status) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (_: Exception) { /* Keep the last known status alongside the reported failure. */ }
            if (failureCode(error).uppercase() in setOf("UNAUTHORIZED", "INVALID_INPUT", "UNSUPPORTED_SCHEMA", "CORRUPT_DATA", "CREDENTIAL")) SyncRun.Blocked
            else SyncRun.Retry
        } finally { active = null; mutable.update { it.copy(running = false) }; schedule() }
    }
    suspend fun beginMaintenance() = withContext(Dispatchers.Main.immediate) {
        maintenance = true
        scheduler.configure(null, mutable.value.wifiOnly)
        command?.cancelAndJoin()
        stopLoop()
        active?.cancelAndJoin()
        gate.withLock { }
    }
    suspend fun endMaintenance() = withContext(Dispatchers.Main.immediate) {
        maintenance = false
        gate.withLock {
            try {
                val binding = repository.binding()
                val status = repository.status(binding)
                val credential = status.origin?.let { loadToken(binding, it) } != null
                mutable.update { it.copy(binding = binding, status = status, hasCredential = credential, loaded = true, error = null) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { failed(error) }
            schedule()
        }
        startLoop()
    }
    private suspend fun loadToken(binding: LibraryBinding, origin: String): String? = try {
        tokens.load(binding.libraryId, origin)
    } catch (cancel: CancellationException) { throw cancel }
    catch (_: Exception) { throw com.grtsinry43.memedock.data.library.LibraryFailure("CREDENTIAL") }
    private fun failed(error: Exception) { mutable.update { it.copy(error = failureCode(error)) } }
}
