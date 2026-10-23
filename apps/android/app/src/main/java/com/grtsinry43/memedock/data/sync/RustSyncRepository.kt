package com.grtsinry43.memedock.data.sync

import com.grtsinry43.memedock.app.LibraryBinding
import com.grtsinry43.memedock.app.LibrarySession
import com.grtsinry43.memedock.bridge.generated.*
import com.grtsinry43.memedock.data.library.LibraryFailure
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.flow

class RustSyncRepository(private val session: LibrarySession) : SyncRepository {
    override val bindings = session.bindings
    override suspend fun binding(): LibraryBinding {
        session.library()
        return session.currentBinding() ?: throw LibraryFailure("CLOSED")
    }
    private suspend fun library(binding: LibraryBinding): LibraryHandle {
        val library = session.library()
        if (session.currentBinding() != binding) throw LibraryFailure("CLOSED")
        return library
    }
    override suspend fun status(binding: LibraryBinding) = run { library(binding).syncStatus() }
    override suspend fun connect(binding: LibraryBinding, origin: String, token: String) = run { library(binding).connectSync(origin, token) }
    override suspend fun synchronize(binding: LibraryBinding, token: String) = run { library(binding).synchronize(token) }
    override suspend fun pause(binding: LibraryBinding, paused: Boolean) = run { library(binding).setSyncPaused(paused) }

    private suspend fun run(create: suspend () -> SyncStatusTask): SyncStatus = withContext(Dispatchers.IO) {
        try {
            create().use { task ->
                try { task.awaitResult().model() }
                catch (cancel: CancellationException) {
                    task.cancel()
                    // The result is single-consumer. Observe terminal progress to drain native work.
                    withContext(NonCancellable) { task.progress().use { progress -> while (progress.next() != null) {} } }
                    throw cancel
                }
            }
        } catch (error: BridgeException.Failure) { throw LibraryFailure(error.code.name) }
    }
    override fun observe(binding: LibraryBinding) = flow {
        val subscription = library(binding).subscribeSync()
        try {
            while (true) emit((subscription.next() ?: break).model())
        } catch (error: BridgeException.Failure) { throw LibraryFailure(error.code.name) }
        finally { try { subscription.unsubscribe() } finally { subscription.close() } }
    }
    private fun com.grtsinry43.memedock.bridge.generated.SyncStatus.model() = SyncStatus(
        origin, deviceId, connected, paused, pending.toLong(), blocked.toLong(), cursor,
        lastSuccessMs, error, retryAfterMs.toLong(),
    )
}
