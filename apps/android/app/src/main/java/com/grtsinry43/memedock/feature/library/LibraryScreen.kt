package com.grtsinry43.memedock.feature.library

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.feature.importing.ImportSourceSheet
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import kotlinx.coroutines.flow.distinctUntilChanged

@Composable
fun LibraryScreen(state: LibraryUiState, imageLoader: ImageLoader, search: (String) -> Unit,
    retry: () -> Unit, more: () -> Unit, thumbnail: (LibraryItem, Boolean) -> Unit,
    pickPhotos: () -> Unit, pickFiles: () -> Unit, importing: Boolean, showImports: () -> Unit, open: (LibraryItem) -> Unit,
    waitingForImport: Boolean = false, showSearch: Boolean = false, title: String = stringResource(R.string.tab_stickers),
    back: (() -> Unit)? = null, toggleStarred: (() -> Unit)? = null, order: (() -> Unit)? = null) {
    val grid = rememberLazyGridState()
    val emptyScroll = rememberScrollState()
    var chooseSource by rememberSaveable { mutableStateOf(false) }
    val add = { if (importing || waitingForImport) showImports() else chooseSource = true }
    LaunchedEffect(grid, state.items.size, state.hasMore, state.loadingMore, state.pageError) {
        snapshotFlow { grid.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: -1 }.distinctUntilChanged().collect { last ->
            if (state.hasMore && !state.loadingMore && state.pageError == null && last >= state.items.size - 6) more()
        }
    }
    Scaffold(containerColor = MaterialTheme.colorScheme.background) { padding ->
        Box(Modifier.fillMaxSize().padding(padding).imePadding(), contentAlignment = Alignment.TopCenter) {
            Column(Modifier.fillMaxSize()) {
                Row(Modifier.fillMaxWidth().padding(horizontal = MemeDockLayout.PagePadding).padding(top = 16.dp, bottom = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (back != null) IconButton(onClick = back) { Icon(MemeDockIcons.Back, stringResource(R.string.back_to_collections)) }
                    Column(Modifier.weight(1f)) {
                        Text(title, style = MaterialTheme.typography.headlineLarge)
                        Text(stringResource(R.string.library_subtitle), style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    FilledTonalButton(onClick = add, contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp)) {
                        Icon(MemeDockIcons.Add, null, Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(stringResource(R.string.library_add))
                    }
                }
                toggleStarred?.let { action ->
                    FilterChip(selected = state.starredOnly, onClick = action, label = { Text(stringResource(R.string.favorites_only)) },
                        modifier = Modifier.padding(horizontal = MemeDockLayout.PagePadding))
                }
                order?.let { TextButton(onClick = it, modifier = Modifier.padding(horizontal = MemeDockLayout.PagePadding)) { Text(stringResource(R.string.order)) } }
                if (showSearch) OutlinedTextField(value = state.search, onValueChange = search, singleLine = true,
                    placeholder = { Text(stringResource(R.string.library_search)) },
                    leadingIcon = { Icon(MemeDockIcons.Search, null) },
                    trailingIcon = { if (state.search.isNotEmpty()) IconButton(onClick = { search("") }) {
                        Icon(MemeDockIcons.Close, stringResource(R.string.clear_search))
                    } }, shape = MaterialTheme.shapes.extraLarge,
                    modifier = Modifier.padding(horizontal = MemeDockLayout.PagePadding).fillMaxWidth().testTag("library-search"))
                AnimatedVisibility(importing || waitingForImport) {
                    Surface(onClick = showImports, shape = MaterialTheme.shapes.large,
                        color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.padding(horizontal = MemeDockLayout.PagePadding).padding(top = 12.dp).fillMaxWidth()) {
                        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            if (importing) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                            else Icon(MemeDockIcons.Image, null, Modifier.size(20.dp))
                            Text(stringResource(if (importing) R.string.import_progress else R.string.import_waiting), style = MaterialTheme.typography.labelLarge)
                        }
                    }
                }
                Spacer(Modifier.height(if (showSearch) 16.dp else 0.dp))
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    when {
                        state.loading -> CircularProgressIndicator(Modifier.size(32.dp), strokeWidth = 3.dp)
                        state.error != null -> MemeDockEmptyState(stringResource(R.string.library_load_title), failureText(state.error),
                            stringResource(R.string.retry), retry, Modifier.verticalScroll(emptyScroll), icon = MemeDockIcons.Alert)
                        state.items.isEmpty() -> if (state.starredOnly) Text(stringResource(R.string.starred_empty))
                        else if (back != null) Column(Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                            Icon(MemeDockIcons.Folder, null, Modifier.size(48.dp), tint = MaterialTheme.colorScheme.primary)
                            Spacer(Modifier.height(16.dp))
                            Text(stringResource(R.string.collection_empty), style = MaterialTheme.typography.titleLarge)
                        } else if (state.search.isBlank()) MemeDockEmptyState(stringResource(R.string.library_empty),
                            stringResource(R.string.library_empty_hint), stringResource(R.string.import_photos), add, Modifier.verticalScroll(emptyScroll))
                        else MemeDockEmptyState(stringResource(R.string.library_no_matches), stringResource(R.string.library_no_matches_hint),
                            stringResource(R.string.clear_search), { search("") }, Modifier.verticalScroll(emptyScroll), icon = MemeDockIcons.Search)
                        else -> LazyVerticalGrid(columns = GridCells.Adaptive(120.dp), state = grid, modifier = Modifier.fillMaxSize().testTag("library-grid"),
                            horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.CardGap), verticalArrangement = Arrangement.spacedBy(0.dp),
                            contentPadding = PaddingValues(bottom = 8.dp)) {
                            items(state.items, key = LibraryItem::id) { item ->
                                LaunchedEffect(item.id, item.thumbnailPath, item.thumbnailState, state.thumbnailEpoch) { thumbnail(item, false) }
                                LibraryTile(item, imageLoader, { thumbnail(item, true) }, { open(item) })
                            }
                            item(span = { GridItemSpan(maxLineSpan) }) {
                                if (state.loadingMore) Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp)) }
                                else if (state.pageError != null) TextButton(onClick = more, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.library_page_error)) }
                            }
                        }
                    }
                }
            }
        }
    }
    if (chooseSource) ImportSourceSheet({ chooseSource = false }, { chooseSource = false; pickPhotos() }, { chooseSource = false; pickFiles() })
}
