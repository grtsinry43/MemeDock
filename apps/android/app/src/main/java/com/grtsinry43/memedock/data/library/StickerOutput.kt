package com.grtsinry43.memedock.data.library

import com.grtsinry43.memedock.data.settings.ExportChoice
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext

/**
 * Exports [id] and hands the artifact to [deliver], which returns true when it takes ownership of
 * the lease. Usage is recorded after delivery even if the caller is cancelled meanwhile; the result
 * is whether the lease was transferred.
 */
suspend fun DetailRepository.output(
    id: String,
    choice: ExportChoice,
    firstFrame: Boolean,
    deliver: suspend (OutputLease, ShareArtifact) -> Boolean,
    record: suspend () -> Unit,
): Boolean {
    var acquired: OutputLease? = null
    try {
        val lease = export(id, choice, firstFrame)
        acquired = lease
        val artifact = prepareHandoff(lease)
        currentCoroutineContext().ensureActive()
        val transferred = deliver(lease, artifact)
        if (transferred) acquired = null
        withContext(NonCancellable) { record() }
        return transferred
    } finally {
        acquired?.close()
    }
}
