package com.grtsinry43.memedock.feature.library

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.res.stringResource
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.failureText
import coil3.ImageLoader
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.ui.components.MemeDockMessageHost
import kotlinx.coroutines.flow.distinctUntilChanged

@Composable
fun LibraryScreen(state: LibraryUiState, imageLoader: ImageLoader, search: (String) -> Unit,
    retry: () -> Unit, more: () -> Unit, thumbnail: (LibraryItem, Boolean) -> Unit,
    pickPhotos: () -> Unit, pickFiles: () -> Unit, importing: Boolean, showImports: () -> Unit) {
    val messages = remember { SnackbarHostState() }
    val grid = rememberLazyGridState()
    LaunchedEffect(grid, state.items.size, state.hasMore, state.loadingMore, state.pageError) {
        snapshotFlow { grid.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: -1 }.distinctUntilChanged().collect { last ->
            if (state.hasMore && !state.loadingMore && state.pageError == null && last >= state.items.size - 6) more()
        }
    }
    Scaffold(snackbarHost = { MemeDockMessageHost(messages) }) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).imePadding().padding(horizontal = 16.dp)) {
            Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineLarge, modifier = Modifier.padding(top = 16.dp))
            Text(stringResource(R.string.library_subtitle), color = MaterialTheme.colorScheme.onSurfaceVariant)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                FilledTonalButton(onClick = if (importing) showImports else pickPhotos, modifier = Modifier.weight(1f)) { Text(stringResource(if (importing) R.string.import_progress else R.string.import_photos)) }
                TextButton(onClick = pickFiles, enabled = !importing, modifier = Modifier.weight(1f)) { Text(stringResource(R.string.import_files)) }
            }
            OutlinedTextField(value = state.search, onValueChange = search, singleLine = true,
                label = { Text(stringResource(R.string.library_search)) }, modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp))
            when {
                state.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
                state.error != null -> Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(stringResource(R.string.library_error, failureText(state.error)))
                    Button(onClick = retry) { Text(stringResource(R.string.retry)) }
                }
                state.items.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text(stringResource(if (state.search.isBlank()) R.string.library_empty else R.string.library_no_matches))
                }
                else -> LazyVerticalGrid(columns = GridCells.Adaptive(112.dp), state = grid,
                    horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp), contentPadding = PaddingValues(bottom = 16.dp)) {
                    items(state.items, key = LibraryItem::id) { item ->
                        LaunchedEffect(item.id, item.thumbnailPath, item.thumbnailState, state.thumbnailEpoch) { thumbnail(item, false) }
                        LibraryTile(item, imageLoader) { thumbnail(item, true) }
                    }
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        if (state.loadingMore) Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
                        else if (state.pageError != null) TextButton(onClick = more, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.library_page_error, failureText(state.pageError))) }
                    }
                }
            }
        }
    }
}
