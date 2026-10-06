package com.grtsinry43.memedock.platform.saving

import android.net.Uri
import com.grtsinry43.memedock.data.library.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.util.concurrent.atomic.AtomicBoolean

data class SaveState(val stickerId: String? = null, val artifact: ShareArtifact? = null,
    val selecting: Boolean = false, val writing: Boolean = false,
    val saved: Boolean = false, val error: String? = null)

/** Owns a prepared lease while the system picker outlives the detail route. */
class SaveCoordinator(private val repository: DetailRepository, private val gateway: SaveGateway,
    private val scope: CoroutineScope) {
    private val mutable = MutableStateFlow(SaveState())
    val state = mutable.asStateFlow()
    private var lease: OutputLease? = null
    private val cancelled = AtomicBoolean(false)
    fun prepare(id: String, acquired: OutputLease, artifact: ShareArtifact) {
        check(lease == null) { "Save already pending" }
        lease = acquired
        cancelled.set(false)
        mutable.value = SaveState(id, artifact)
    }
    fun selecting(): ShareArtifact? {
        val current = mutable.value
        if (current.selecting || current.writing) return null
        val artifact = current.artifact ?: return null
        mutable.value = current.copy(selecting = true)
        return artifact
    }
    fun cancel() { cancelled.set(true) }
    fun clearFeedback() { if (lease == null) mutable.value = SaveState() }
    fun launchFailed(error: Exception) {
        release()
        mutable.update { it.copy(artifact = null, selecting = false, error = reason(error)) }
    }
    fun selected(destination: Uri?) {
        val current = mutable.value
        val output = lease
        if (destination == null) {
            release()
            mutable.value = SaveState(current.stickerId)
            return
        }
        if (output == null || current.artifact == null || current.stickerId == null) {
            // Process death can leave a newly created document without its lease.
            scope.launch { runCatching { gateway.discardCreatedDocument(destination) } }
            return
        }
        if (current.writing) return
        mutable.value = current.copy(selecting = false, writing = true)
        scope.launch {
            try {
                // Explicit cancellation is polled during streaming. Once close
                // succeeds, accounting finishes even if the activity disappears.
                withContext(NonCancellable) {
                    gateway.save(destination, current.artifact, cancelled::get)
                    mutable.update { it.copy(saved = true) }
                    repository.recordSaved(current.stickerId)
                }
            } catch (_: CancellationException) { /* User cancelled the copy. */ }
            catch (error: Exception) { mutable.update { it.copy(error = reason(error)) } }
            finally {
                release()
                mutable.update { it.copy(artifact = null, writing = false) }
            }
        }
    }
    private fun release() { lease?.close(); lease = null }
    private fun reason(error: Exception) = when (error) {
        is LibraryFailure -> error.reason
        is SecurityException -> "PERMISSION_DENIED"
        is java.io.IOException -> "IO"
        is android.content.ActivityNotFoundException -> "NO_SAVE_TARGET"
        else -> "INTERNAL"
    }
}
