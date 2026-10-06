package com.grtsinry43.memedock.data.library

import com.grtsinry43.memedock.app.LibrarySession
import com.grtsinry43.memedock.bridge.generated.*
import kotlinx.coroutines.*

/** Archive ownership stays here; UI only sees summaries and controlled paths. */
class BackupRepository(private val session: LibrarySession) {
    suspend fun create(): BackupFileHandle = withContext(Dispatchers.IO) { translate {
        session.library().createBackup().use { task ->
            try { task.awaitResult() } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    suspend fun input(): ArchiveInputHandle = withContext(Dispatchers.IO) { translate {
        session.library().createArchiveInput().use { it.awaitResult() }
    } }
    suspend fun inspect(input: ArchiveInputHandle, onTask: (ArchiveInspectionTask?) -> Unit): PreparedArchiveHandle = withContext(Dispatchers.IO) { translate {
        session.library().inspectArchive(input).use { task ->
            onTask(task)
            try { task.awaitResult() } catch (cancel: CancellationException) { task.cancel(); throw cancel }
            finally { onTask(null) }
        }
    } }
    suspend fun discard(value: BackupFileHandle) = withContext(NonCancellable + Dispatchers.IO) {
        try { translate { session.library().discardBackup(value).use { it.awaitResult() } } } finally { value.close() }
    }
    suspend fun discard(value: ArchiveInputHandle) = withContext(NonCancellable + Dispatchers.IO) {
        try { translate { session.library().discardArchiveInput(value).use { it.awaitResult() } } } finally { value.close() }
    }
    suspend fun discard(value: PreparedArchiveHandle) = withContext(NonCancellable + Dispatchers.IO) {
        try { translate { session.library().discardPreparedArchive(value).use { it.awaitResult() } } } finally { value.close() }
    }
    suspend fun restore(value: PreparedArchiveHandle, mode: ArchiveRestoreMode, onTask: (ArchiveRestoreTask?) -> Unit): ArchiveSummary =
        withContext(NonCancellable + Dispatchers.IO) {
            try { translate {
                session.library().restoreArchive(value, mode).use { task ->
                    onTask(task)
                    try { task.awaitResult() } finally { onTask(null) }
                }
            } } finally {
                if (mode == ArchiveRestoreMode.REPLACE) session.reconnect()
            }
        }
    private suspend fun <T> translate(block: suspend () -> T): T = try { block() }
        catch (failure: BridgeException.Failure) { throw LibraryFailure(failure.code.name) }
}
