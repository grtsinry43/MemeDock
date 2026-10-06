package com.grtsinry43.memedock.feature.importing

import com.grtsinry43.memedock.platform.importing.ImportCandidate
import com.grtsinry43.memedock.data.library.LibraryCollection

enum class ImportPhase { Preparing, Review, Running, Finished }
enum class ImportItemStatus { Queued, Staged, Reading, Validating, Created, Reused, RestoreRequired, Failed, Cancelled }
data class ImportItemState(
    val candidate: ImportCandidate,
    val status: ImportItemStatus = ImportItemStatus.Queued,
    val bytesRead: Long = 0,
    val error: String? = null,
    /** Our own copy while the item waits in staging; previews read it instead of the source again. */
    val stagedPath: String? = null,
)
data class ImportState(
    val collection: LibraryCollection? = null,
    val phase: ImportPhase = ImportPhase.Review,
    val items: List<ImportItemState> = emptyList(),
    val visible: Boolean = false,
    val cancellationRequested: Boolean = false,
    val selectionError: String? = null,
) {
    val busy get() = phase == ImportPhase.Preparing || phase == ImportPhase.Running
    val ready get() = items.count { it.status == ImportItemStatus.Queued || it.status == ImportItemStatus.Staged }
    val unresolved get() = items.count { it.status in Unresolved }
    val retryable get() = items.any { it.status == ImportItemStatus.Failed || it.status == ImportItemStatus.Cancelled }
    fun count(status: ImportItemStatus) = items.count { it.status == status }
    companion object {
        val Unresolved = setOf(ImportItemStatus.Failed, ImportItemStatus.Cancelled, ImportItemStatus.RestoreRequired)
    }
}

data class ImportReport(val created: Int, val reused: Int, val unresolved: Int, val stopped: Boolean)
