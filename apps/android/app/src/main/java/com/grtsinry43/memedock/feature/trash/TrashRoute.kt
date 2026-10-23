package com.grtsinry43.memedock.feature.trash

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.library.trashEntry
import com.grtsinry43.memedock.feature.library.LibraryViewModel
import com.grtsinry43.memedock.feature.library.StickerGrid
import com.grtsinry43.memedock.feature.library.StickerGridActions
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureTextRes
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import dev.chrisbanes.haze.rememberHazeState

/** Deleted collections and tags restore in place; a deleted sticker opens its detail page, which restores it. */
@Composable
fun TrashRoute(container: AppContainer, back: () -> Unit, open: (LibraryItem) -> Unit) {
    val stickers: LibraryViewModel = viewModel(key = "library:trash", factory = factory {
        LibraryViewModel(container.library, deleted = true)
    })
    val model: TrashViewModel = viewModel(factory = factory { TrashViewModel(container.library, container.library.changes) })
    val items by stickers.state.collectAsStateWithLifecycle()
    val state by model.state.collectAsStateWithLifecycle()
    val messages = LocalMemeDockMessages.current
    val resources = LocalContext.current.resources
    LaunchedEffect(model) {
        model.events.collect { event ->
            messages.showMemeDockMessage(when (event) {
                TrashEvent.Restored -> MemeDockMessage(resources.getString(R.string.restored), MemeDockMessageType.Success)
                is TrashEvent.Failed -> MemeDockMessage(resources.getString(failureTextRes(event.reason)), MemeDockMessageType.Error)
            })
        }
    }
    val batchModel = com.grtsinry43.memedock.feature.library.rememberBatchActions(container, "trash", items.search)
    val selection by batchModel.state.collectAsStateWithLifecycle()
    var batchSheet by remember { mutableStateOf(false) }
    com.grtsinry43.memedock.feature.library.BatchActionsSheet(batchSheet, { batchSheet = false }, batchModel, container.library,
        trash = true, purge = { targets -> container.trash.request(targets.map { it.trashEntry() }, batchModel::removePurged) })
    val purge by container.trash.state.collectAsStateWithLifecycle()
    val grid = rememberLazyGridState()
    val glass = rememberHazeState()
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + MemeDockLayout.TopBarHeight
    val bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val title = stringResource(R.string.trash)
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        StickerGrid(
            items, container.imageLoader,
            StickerGridActions(stickers::ensureThumbnail, { if (selection.selecting) batchModel.toggle(it) else open(it) },
                { batchModel.start(it) }, stickers::loadMore, stickers::retry, selection, { batchModel.start() }, batchModel::exit, { batchSheet = true }),
            grid, Modifier.glassSource(glass), PaddingValues(top = top + 8.dp, bottom = bottom),
            header = {
                item(key = "trash-title", span = { GridItemSpan(maxLineSpan) }, contentType = "title") {
                    Text(title, style = MaterialTheme.typography.headlineLarge,
                        modifier = Modifier.padding(start = 4.dp, end = 4.dp, top = 12.dp, bottom = 8.dp).semantics { heading() })
                }
                if (state.collections.isNotEmpty()) item(key = "trash-collections", span = { GridItemSpan(maxLineSpan) }) {
                    RestoreGroup(stringResource(R.string.tab_collections), MemeDockIcons.Folder,
                        state.collections.map { value -> RestoreEntry(value.id, value.name, { model.restore(value) },
                            { container.trash.request(listOf(value.trashEntry())) }) }, state.restoring, purge.busy || purge.request != null)
                }
                if (state.tags.isNotEmpty()) item(key = "trash-tags", span = { GridItemSpan(maxLineSpan) }) {
                    RestoreGroup(stringResource(R.string.tags_title), MemeDockIcons.Label,
                        state.tags.map { value -> RestoreEntry(value.id, value.name, { model.restore(value) },
                            { container.trash.request(listOf(value.trashEntry())) }) }, state.restoring, purge.busy || purge.request != null)
                }
                if (items.items.isNotEmpty()) item(key = "trash-stickers", span = { GridItemSpan(maxLineSpan) }) {
                    Column(Modifier.padding(start = 16.dp, top = 8.dp, bottom = 4.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(stringResource(R.string.tab_stickers), style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.semantics { heading() })
                        Text(stringResource(R.string.trash_stickers_hint), style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            },
            empty = {
                if (state.collections.isEmpty() && state.tags.isEmpty()) MemeDockEmptyState(stringResource(R.string.trash_empty),
                    stringResource(R.string.trash_empty_hint), icon = MemeDockIcons.Delete)
            },
        )
        val scrolled by remember { derivedStateOf { grid.canScrollBackward } }
        val titled by remember { derivedStateOf { grid.firstVisibleItemIndex > 0 } }
        MemeDockTopBar(if (titled) title else "", glass, back = back, scrolled = scrolled) {
            var more by remember { mutableStateOf(false) }
            Box {
                IconButton({ more = true }, enabled = !purge.busy, modifier = Modifier.testTag("trash-more")) {
                    Icon(MemeDockIcons.More, stringResource(R.string.more_actions))
                }
                DropdownMenu(more, { more = false }) {
                    DropdownMenuItem(text = { Text(stringResource(R.string.purge_empty)) },
                        onClick = { more = false; container.trash.empty() }, modifier = Modifier.testTag("trash-empty"))
                    DropdownMenuItem(text = { Text(stringResource(R.string.purge_cleanup)) },
                        onClick = { more = false; container.trash.retryCleanup() })
                }
            }
            com.grtsinry43.memedock.feature.library.SelectionModeButton(StickerGridActions(
                stickers::ensureThumbnail, open, null, stickers::loadMore, stickers::retry,
                selection, { batchModel.start() }, batchModel::exit))
        }
    }
}

private class RestoreEntry(val id: String, val name: String, val restore: () -> Unit, val purge: () -> Unit)

@Composable
private fun RestoreGroup(title: String, icon: ImageVector, entries: List<RestoreEntry>, restoring: String?, purging: Boolean) {
    MemeDockGroup(Modifier.padding(top = 8.dp, bottom = 8.dp), title = title, horizontalPadding = 0.dp) {
        entries.forEach { entry ->
            row {
                MemeDockRow(entry.name, icon = icon, trailing = {
                    if (restoring == entry.id) CircularProgressIndicator(Modifier.padding(horizontal = 12.dp).size(MemeDockLayout.IconMedium), strokeWidth = 2.dp)
                    else Row {
                        TextButton(onClick = entry.restore, enabled = restoring == null && !purging,
                            modifier = Modifier.testTag("restore:${entry.id}")) { Text(stringResource(R.string.restore)) }
                        IconButton(entry.purge, enabled = restoring == null && !purging, modifier = Modifier.testTag("purge:${entry.id}")) {
                            Icon(MemeDockIcons.Delete, stringResource(R.string.purge_title), tint = MaterialTheme.colorScheme.error)
                        }
                    }
                })
            }
        }
    }
}

private inline fun <reified M : ViewModel> factory(crossinline create: () -> M) = object : ViewModelProvider.Factory {
    override fun <T : ViewModel> create(modelClass: Class<T>): T = modelClass.cast(create())!!
}
