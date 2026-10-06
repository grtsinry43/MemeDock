package com.grtsinry43.memedock.feature.backup

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.bridge.generated.ArchiveRestoreMode
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureTextRes
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import dev.chrisbanes.haze.rememberHazeState
import kotlinx.coroutines.launch

private val CreatePhases = setOf(BackupPhase.Creating, BackupPhase.ChooseDestination, BackupPhase.Saving)
private val RestorePhases = setOf(BackupPhase.ChooseSource, BackupPhase.Reading, BackupPhase.Restoring)

@Composable
fun BackupRoute(coordinator: BackupCoordinator, back: () -> Unit) {
    val state by coordinator.state.collectAsStateWithLifecycle()
    var replacing by rememberSaveable { mutableStateOf(false) }
    val messages = LocalMemeDockMessages.current
    val resources = LocalContext.current.resources
    val scope = rememberCoroutineScope()
    // A replace swaps the library and recreates this page, so the result is read from the coordinator, not remembered.
    LaunchedEffect(state.phase, state.error) {
        val error = state.error
        val message = when {
            state.phase == BackupPhase.Saved -> MemeDockMessage(resources.getString(R.string.backup_saved), MemeDockMessageType.Success)
            state.phase == BackupPhase.Restored -> MemeDockMessage(resources.getString(R.string.backup_restored), MemeDockMessageType.Success)
            error != null -> MemeDockMessage(resources.getString(backupFailureRes(error)), MemeDockMessageType.Error)
            else -> return@LaunchedEffect
        }
        coordinator.acknowledge()
        // Acknowledging restarts this effect; the message must not be cancelled with it.
        scope.launch { messages.showMemeDockMessage(message) }
    }
    val scroll = rememberScrollState()
    val glass = rememberHazeState()
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + MemeDockLayout.TopBarHeight
    val bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val title = stringResource(R.string.backup_title)
    val previewing = state.phase == BackupPhase.Preview
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        Column(Modifier.fillMaxSize().glassSource(glass).verticalScroll(scroll)
            .padding(top = top + 8.dp, bottom = bottom + MemeDockLayout.SectionGap)) {
            Text(title, style = MaterialTheme.typography.headlineLarge,
                modifier = Modifier.padding(horizontal = MemeDockLayout.PagePadding, vertical = 12.dp).semantics { heading() })
            MemeDockGroup(title = stringResource(R.string.backup_heading), footer = stringResource(R.string.backup_description)) {
                row {
                    if (state.phase in CreatePhases) Progress(state.phase, coordinator::cancel)
                    else MemeDockRow(stringResource(R.string.backup_create), Modifier.testTag("backup-create"), icon = MemeDockIcons.Archive,
                        enabled = !state.busy && !previewing, onClick = coordinator::create)
                }
            }
            Spacer(Modifier.height(MemeDockLayout.SectionGap))
            MemeDockGroup(title = stringResource(R.string.backup_restore_heading),
                footer = stringResource(R.string.backup_restore_description)) {
                row {
                    if (state.phase in RestorePhases) Progress(state.phase, coordinator::cancel)
                    else MemeDockRow(stringResource(R.string.backup_choose), Modifier.testTag("backup-choose"), icon = MemeDockIcons.Unarchive,
                        enabled = !state.busy && !previewing, onClick = coordinator::chooseSource)
                }
            }
            if (previewing) state.summary?.let { summary ->
                Spacer(Modifier.height(MemeDockLayout.SectionGap))
                MemeDockGroup(title = stringResource(R.string.backup_preview),
                    footer = stringResource(R.string.backup_merge_summary, summary.added.toLong(), summary.preserved.toLong())) {
                    row { MemeDockRow(stringResource(R.string.tab_stickers), value = (summary.stickers - summary.deletedStickers).toString()) }
                    row { MemeDockRow(stringResource(R.string.trash), value = summary.deletedStickers.toString()) }
                    row { MemeDockRow(stringResource(R.string.tab_collections), value = summary.collections.toString()) }
                    row { MemeDockRow(stringResource(R.string.tags_title), value = summary.tags.toString()) }
                    row { MemeDockRow(stringResource(R.string.backup_originals), value = formatFileSize(summary.originalBytes.toLong())) }
                }
                PreviewActions(
                    merge = { coordinator.restore(ArchiveRestoreMode.MERGE) },
                    replace = { replacing = true },
                    cancel = coordinator::dismissPreview,
                )
            }
        }
        val scrolled by remember { derivedStateOf { scroll.value > 0 } }
        val density = LocalDensity.current
        val titled by remember { derivedStateOf { scroll.value > with(density) { 56.dp.roundToPx() } } }
        MemeDockTopBar(if (titled) title else "", glass, back = back, scrolled = scrolled)
    }
    MemeDockConfirmSheet(
        visible = replacing && (previewing || state.phase == BackupPhase.Restoring),
        title = stringResource(R.string.backup_replace_confirm),
        message = stringResource(R.string.backup_replace_hint),
        confirmLabel = stringResource(R.string.backup_replace),
        onConfirm = { coordinator.restore(ArchiveRestoreMode.REPLACE) },
        onDismissRequest = { replacing = false },
        destructive = true,
        busy = state.phase == BackupPhase.Restoring,
    )
}

@Composable
private fun Progress(phase: BackupPhase, cancel: () -> Unit) {
    val waitingForPicker = phase == BackupPhase.ChooseDestination || phase == BackupPhase.ChooseSource
    Row(Modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        Box(Modifier.size(22.dp), contentAlignment = Alignment.Center) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
        }
        Text(stringResource(when (phase) {
            BackupPhase.Creating -> R.string.backup_creating
            BackupPhase.Saving -> R.string.backup_saving
            BackupPhase.Reading -> R.string.backup_reading
            BackupPhase.Restoring -> R.string.backup_restoring
            else -> R.string.backup_selecting
        }), Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (!waitingForPicker) TextButton(onClick = cancel) { Text(stringResource(R.string.cancel)) }
    }
}

@Composable
private fun PreviewActions(merge: () -> Unit, replace: () -> Unit, cancel: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().padding(horizontal = MemeDockLayout.PagePadding).padding(top = 16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(stringResource(R.string.backup_merge_hint), style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        Button(onClick = merge, modifier = Modifier.fillMaxWidth().padding(top = 8.dp).height(52.dp).testTag("backup-merge"),
            shape = MaterialTheme.shapes.medium) { Text(stringResource(R.string.backup_merge)) }
        TextButton(onClick = replace, modifier = Modifier.fillMaxWidth().height(52.dp).testTag("backup-replace"),
            shape = MaterialTheme.shapes.medium) { Text(stringResource(R.string.backup_replace_action), color = colors.error) }
        TextButton(onClick = cancel, modifier = Modifier.fillMaxWidth().height(52.dp), shape = MaterialTheme.shapes.medium) {
            Text(stringResource(R.string.cancel), color = colors.onSurfaceVariant)
        }
    }
}

private fun backupFailureRes(code: String) = when (code) {
    "CORRUPT_DATA", "INVALID_IMAGE" -> R.string.backup_invalid
    "UNSUPPORTED_FORMAT" -> R.string.backup_unsupported
    else -> failureTextRes(code)
}
