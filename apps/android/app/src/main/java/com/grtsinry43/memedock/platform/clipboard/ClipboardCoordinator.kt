package com.grtsinry43.memedock.platform.clipboard

import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

class ClipboardCoordinator(private val repository: ClipboardRepository,
    private val gateway: ClipboardGateway, private val scope: CoroutineScope) {
    private val mutex = Mutex()
    suspend fun copy(lease: OutputLease, artifact: ShareArtifact) = mutex.withLock {
        // No cancellation between durable protection and the observable handoff.
        withContext(NonCancellable) {
            val reference = repository.protectClipboard(lease)
            try { gateway.copy(artifact, reference) }
            catch (error: Exception) {
                try { repository.abortClipboard(reference) } catch (cleanup: Exception) { error.addSuppressed(cleanup) }
                throw error
            }
            try { repository.reconcileClipboard(reference) }
            catch (error: LibraryFailure) {
                // Copy already succeeded; the pending pin is deliberately kept.
                android.util.Log.w("MemeDockClipboard", "Clipboard confirmation deferred: ${error.reason}")
            }
        }
    }
    fun foreground(focused: () -> Boolean) {
        scope.launch {
            mutex.withLock {
                if (!focused()) return@withLock
                // Failed reads must retain protection rather than guessing empty.
                val observed = try { gateway.observedReference() } catch (_: SecurityException) { return@withLock }
                try { repository.reconcileClipboard(observed) }
                catch (cancel: CancellationException) { throw cancel }
                catch (_: LibraryFailure) { /* Keep durable pins and retry next focus. */ }
            }
        }
    }
}
