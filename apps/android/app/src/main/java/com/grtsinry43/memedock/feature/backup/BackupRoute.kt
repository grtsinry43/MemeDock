package com.grtsinry43.memedock.feature.backup

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.bridge.generated.ArchiveRestoreMode
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.components.formatFileSize
import com.grtsinry43.memedock.ui.failureText

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BackupRoute(coordinator: BackupCoordinator, back: () -> Unit) {
    val state by coordinator.state.collectAsStateWithLifecycle()
    var replace by rememberSaveable { mutableStateOf(false) }
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        TopAppBar(title = { Text(stringResource(R.string.backup_title)) }, navigationIcon = {
            IconButton(onClick = back) { Icon(MemeDockIcons.Back, stringResource(R.string.back_to_library)) }
        })
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp)) {
            Surface(shape = MaterialTheme.shapes.extraLarge, color = MaterialTheme.colorScheme.primaryContainer) {
                Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(stringResource(R.string.backup_heading), style = MaterialTheme.typography.headlineSmall)
                    Text(stringResource(R.string.backup_description), style = MaterialTheme.typography.bodyLarge)
                    Button(onClick = coordinator::create, enabled = !state.busy && state.phase != BackupPhase.Preview) {
                        Text(stringResource(R.string.backup_create))
                    }
                }
            }
            if (state.busy) {
                Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow) {
                    Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        LinearProgressIndicator(Modifier.fillMaxWidth())
                        Text(stringResource(when (state.phase) {
                            BackupPhase.Creating -> R.string.backup_creating
                            BackupPhase.Saving -> R.string.backup_saving
                            BackupPhase.Reading -> R.string.backup_reading
                            BackupPhase.Restoring -> R.string.backup_restoring
                            else -> R.string.backup_selecting
                        }))
                        if (state.phase !in listOf(BackupPhase.ChooseDestination, BackupPhase.ChooseSource)) {
                            TextButton(onClick = coordinator::cancel) { Text(stringResource(R.string.cancel)) }
                        }
                    }
                }
            }
            if (state.phase == BackupPhase.Saved || state.phase == BackupPhase.Restored) {
                Text(stringResource(if (state.phase == BackupPhase.Saved) R.string.backup_saved else R.string.backup_restored),
                    color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.titleMedium)
            }
            state.error?.let {
                val text = when (it) {
                    "CORRUPT_DATA", "INVALID_IMAGE" -> stringResource(R.string.backup_invalid)
                    "UNSUPPORTED_FORMAT" -> stringResource(R.string.backup_unsupported)
                    else -> failureText(it)
                }
                Text(text, color = MaterialTheme.colorScheme.error)
            }
            Text(stringResource(R.string.backup_restore_heading), style = MaterialTheme.typography.titleLarge)
            Text(stringResource(R.string.backup_restore_description))
            OutlinedButton(onClick = coordinator::chooseSource, enabled = !state.busy && state.phase != BackupPhase.Preview,
                modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.backup_choose)) }
            if (state.phase == BackupPhase.Preview) state.summary?.let { summary ->
                Surface(shape = MaterialTheme.shapes.extraLarge, color = MaterialTheme.colorScheme.surfaceContainerLow) {
                    Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                        Text(stringResource(R.string.backup_preview), style = MaterialTheme.typography.titleMedium)
                        SummaryRow(stringResource(R.string.tab_stickers), (summary.stickers - summary.deletedStickers).toString())
                        SummaryRow(stringResource(R.string.trash), summary.deletedStickers.toString())
                        SummaryRow(stringResource(R.string.tab_collections), summary.collections.toString())
                        SummaryRow(stringResource(R.string.manage_tags), summary.tags.toString())
                        SummaryRow(stringResource(R.string.backup_originals), formatFileSize(summary.originalBytes.toLong()))
                        HorizontalDivider()
                        Text(stringResource(R.string.backup_merge_hint))
                        Text(stringResource(R.string.backup_merge_summary, summary.added.toLong(), summary.preserved.toLong()),
                            color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
                        Button(onClick = { coordinator.restore(ArchiveRestoreMode.MERGE) }, modifier = Modifier.fillMaxWidth()) {
                            Text(stringResource(R.string.backup_merge))
                        }
                        TextButton(onClick = { replace = true }, modifier = Modifier.fillMaxWidth()) {
                            Text(stringResource(R.string.backup_replace))
                        }
                        TextButton(onClick = coordinator::dismissPreview, modifier = Modifier.fillMaxWidth()) {
                            Text(stringResource(R.string.cancel))
                        }
                    }
                }
            }
        }
    }
    if (replace && state.phase == BackupPhase.Preview) AlertDialog(onDismissRequest = { replace = false },
        title = { Text(stringResource(R.string.backup_replace_confirm)) },
        text = { Text(stringResource(R.string.backup_replace_hint)) },
        dismissButton = { TextButton(onClick = { replace = false }) { Text(stringResource(R.string.cancel)) } },
        confirmButton = { TextButton(onClick = { replace = false; coordinator.restore(ArchiveRestoreMode.REPLACE) }) {
            Text(stringResource(R.string.backup_replace))
        } })
}

@Composable
private fun SummaryRow(label: String, value: String) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(label, Modifier.weight(1f), color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.labelLarge)
    }
}
