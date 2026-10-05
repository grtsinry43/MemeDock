package com.grtsinry43.memedock.feature.importing

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.res.stringResource
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.failureText

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ImportSheet(state: ImportState, coordinator: ImportCoordinator) {
    ModalBottomSheet(onDismissRequest = coordinator::hide, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().heightIn(max = 600.dp).padding(horizontal = 20.dp).padding(bottom = 20.dp)) {
            Text(stringResource(if (state.phase == ImportPhase.Finished) R.string.import_results else R.string.import_photos), style = MaterialTheme.typography.headlineSmall)
            Text(stringResource(R.string.import_limits), style = MaterialTheme.typography.bodySmall)
            state.selectionError?.let { Text(stringResource(R.string.import_selection_error, failureText(it)), color = MaterialTheme.colorScheme.error) }
            if (state.phase == ImportPhase.Preparing) LinearProgressIndicator(Modifier.fillMaxWidth().padding(vertical = 12.dp))
            LazyColumn(Modifier.fillMaxWidth().weight(1f, fill = false).padding(vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                items(state.items, key = { it.candidate.uri }) { item ->
                    Column {
                        Text(item.candidate.name, style = MaterialTheme.typography.titleSmall)
                        Text(when (item.status) {
                            ImportItemStatus.Queued -> stringResource(R.string.import_queued)
                            ImportItemStatus.Reading -> stringResource(R.string.import_reading, item.bytesRead / 1024)
                            ImportItemStatus.Validating -> stringResource(R.string.import_validating)
                            ImportItemStatus.Created -> stringResource(R.string.import_created)
                            ImportItemStatus.Reused -> stringResource(R.string.import_reused)
                            ImportItemStatus.RestoreRequired -> stringResource(R.string.import_restore_required)
                            ImportItemStatus.Failed -> stringResource(R.string.import_failed, failureText(item.error))
                            ImportItemStatus.Cancelled -> stringResource(R.string.import_cancelled)
                        }, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
            if (state.phase == ImportPhase.Finished) Text(stringResource(R.string.import_summary, state.count(ImportItemStatus.Created), state.count(ImportItemStatus.Reused), state.count(ImportItemStatus.Failed), state.count(ImportItemStatus.Cancelled)))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                when (state.phase) {
                    ImportPhase.Review -> {
                        val queued = state.items.any { it.status == ImportItemStatus.Queued }
                        Button(onClick = if (queued) coordinator::start else coordinator::retryFailed, modifier = Modifier.weight(1f),
                            enabled = queued || state.items.any { it.status == ImportItemStatus.Failed }) {
                            Text(stringResource(if (queued) R.string.import_start else R.string.import_retry_failed))
                        }
                    }
                    ImportPhase.Running -> OutlinedButton(onClick = coordinator::cancel, modifier = Modifier.weight(1f), enabled = !state.cancellationRequested) { Text(stringResource(if (state.cancellationRequested) R.string.import_cancelling else R.string.import_cancel_remaining)) }
                    ImportPhase.Finished -> if (state.items.any { it.status == ImportItemStatus.Failed || it.status == ImportItemStatus.Cancelled }) Button(onClick = coordinator::retryFailed, modifier = Modifier.weight(1f)) { Text(stringResource(R.string.import_retry_failed)) }
                    ImportPhase.Preparing -> Unit
                }
                TextButton(onClick = coordinator::hide, modifier = Modifier.weight(1f)) { Text(stringResource(if (state.busy) R.string.collapse else R.string.done)) }
            }
        }
    }
}
