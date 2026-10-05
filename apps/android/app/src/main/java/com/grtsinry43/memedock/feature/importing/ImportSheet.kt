package com.grtsinry43.memedock.feature.importing

import androidx.compose.animation.AnimatedContent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.failureText

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ImportSheet(state: ImportState, coordinator: ImportCoordinator) {
    ModalBottomSheet(onDismissRequest = coordinator::hide, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        ImportSheetContent(state, coordinator::start, coordinator::retryFailed, coordinator::cancel, coordinator::hide, coordinator::discard)
    }
}

@Composable
fun ImportSheetContent(state: ImportState, start: () -> Unit, retry: () -> Unit, cancel: () -> Unit, hide: () -> Unit, discard: () -> Unit) {
    val unresolved = state.items.count { it.status in listOf(ImportItemStatus.Failed, ImportItemStatus.Cancelled, ImportItemStatus.RestoreRequired) }
    val completed = state.items.count { it.status in listOf(ImportItemStatus.Created, ImportItemStatus.Reused, ImportItemStatus.RestoreRequired, ImportItemStatus.Failed, ImportItemStatus.Cancelled) }
    val canRetry = state.items.any { it.status == ImportItemStatus.Failed || it.status == ImportItemStatus.Cancelled }
    Column(Modifier.fillMaxWidth().heightIn(max = 650.dp).padding(horizontal = 24.dp).padding(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)) {
        AnimatedContent(state.phase, label = "ImportHeading") { phase ->
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(when (phase) {
                    ImportPhase.Preparing -> R.string.import_preparing_title
                    ImportPhase.Running -> R.string.import_running_title
                    ImportPhase.Review -> R.string.import_photos
                    ImportPhase.Finished -> R.string.import_results
                }), style = MaterialTheme.typography.headlineSmall)
                Text(stringResource(when (phase) {
                    ImportPhase.Preparing, ImportPhase.Running -> R.string.import_running_hint
                    ImportPhase.Review -> R.string.import_review_hint
                    ImportPhase.Finished -> if (unresolved == 0) R.string.import_finished_hint else R.string.import_finished_partial_hint
                }), color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
            }
        }
        state.selectionError?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium) }
        if (state.phase == ImportPhase.Finished) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ImportCount(state.count(ImportItemStatus.Created), stringResource(R.string.import_summary_created), Modifier.weight(1f))
                ImportCount(state.count(ImportItemStatus.Reused), stringResource(R.string.import_summary_reused), Modifier.weight(1f))
                ImportCount(unresolved, stringResource(R.string.import_summary_failed), Modifier.weight(1f))
            }
        } else {
            Text(if (state.busy) stringResource(R.string.import_completion, completed, state.items.size)
                else stringResource(R.string.import_count, state.items.size), style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary)
            if (state.phase == ImportPhase.Preparing) LinearProgressIndicator(Modifier.fillMaxWidth())
            else if (state.phase == ImportPhase.Running && state.items.isNotEmpty())
                LinearProgressIndicator(progress = { completed.toFloat() / state.items.size }, modifier = Modifier.fillMaxWidth())
        }
        LazyColumn(Modifier.fillMaxWidth().weight(1f, fill = false), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(state.items, key = { it.candidate.uri }) { item ->
                val bad = item.status == ImportItemStatus.Failed || item.status == ImportItemStatus.RestoreRequired
                val success = item.status == ImportItemStatus.Created || item.status == ImportItemStatus.Reused
                Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow) {
                    Row(Modifier.fillMaxWidth().padding(14.dp), verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        if (item.status == ImportItemStatus.Reading || item.status == ImportItemStatus.Validating)
                            CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                        else Icon(if (bad) MemeDockIcons.Alert else if (success) MemeDockIcons.Check else MemeDockIcons.Image,
                            null, Modifier.size(20.dp), tint = if (bad) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary)
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Text(item.candidate.name, style = MaterialTheme.typography.titleSmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
                            Text(when (item.status) {
                                ImportItemStatus.Queued -> stringResource(R.string.import_queued)
                                ImportItemStatus.Staged -> stringResource(R.string.import_staged)
                                ImportItemStatus.Reading -> stringResource(R.string.import_reading)
                                ImportItemStatus.Validating -> stringResource(R.string.import_validating)
                                ImportItemStatus.Created -> stringResource(R.string.import_created)
                                ImportItemStatus.Reused -> stringResource(R.string.import_reused)
                                ImportItemStatus.RestoreRequired -> stringResource(R.string.import_restore_required)
                                ImportItemStatus.Failed -> failureText(item.error)
                                ImportItemStatus.Cancelled -> stringResource(R.string.import_cancelled)
                            }, style = MaterialTheme.typography.bodySmall,
                                color = if (bad) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
        when (state.phase) {
            ImportPhase.Review -> {
                val ready = state.items.any { it.status == ImportItemStatus.Queued || it.status == ImportItemStatus.Staged }
                Button(onClick = if (ready) start else retry, enabled = ready || canRetry, modifier = Modifier.fillMaxWidth()) {
                    Text(stringResource(if (ready) R.string.import_start else R.string.import_retry_failed))
                }
                if (state.items.isNotEmpty()) TextButton(onClick = discard, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.import_discard)) }
                else TextButton(onClick = hide, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.done)) }
            }
            ImportPhase.Preparing, ImportPhase.Running -> {
                Button(onClick = hide, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.import_background)) }
                TextButton(onClick = cancel, enabled = !state.cancellationRequested, modifier = Modifier.fillMaxWidth()) {
                    Text(stringResource(if (state.cancellationRequested) R.string.import_cancelling else R.string.import_cancel_remaining))
                }
            }
            ImportPhase.Finished -> {
                Button(onClick = hide, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.done)) }
                if (canRetry) TextButton(onClick = retry, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.import_retry_failed)) }
            }
        }
    }
}

@Composable
private fun ImportCount(count: Int, label: String, modifier: Modifier) {
    Surface(modifier = modifier, shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.secondaryContainer) {
        Column(Modifier.padding(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(count.toString(), style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onSecondaryContainer)
            Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSecondaryContainer)
        }
    }
}
