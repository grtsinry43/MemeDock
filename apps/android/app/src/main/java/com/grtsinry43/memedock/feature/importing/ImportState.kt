package com.grtsinry43.memedock.feature.importing

import com.grtsinry43.memedock.platform.importing.ImportCandidate

enum class ImportPhase { Preparing, Review, Running, Finished }
enum class ImportItemStatus { Queued, Staged, Reading, Validating, Created, Reused, RestoreRequired, Failed, Cancelled }
data class ImportItemState(
    val candidate: ImportCandidate,
    val status: ImportItemStatus = ImportItemStatus.Queued,
    val bytesRead: Long = 0,
    val error: String? = null,
)
data class ImportState(
    val phase: ImportPhase = ImportPhase.Review,
    val items: List<ImportItemState> = emptyList(),
    val visible: Boolean = false,
    val cancellationRequested: Boolean = false,
    val selectionError: String? = null,
) {
    val busy get() = phase == ImportPhase.Preparing || phase == ImportPhase.Running
    fun count(status: ImportItemStatus) = items.count { it.status == status }
}
