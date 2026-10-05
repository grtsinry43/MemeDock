package com.grtsinry43.memedock.feature.collections

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryCollection
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText

@Composable
fun CollectionsScreen(state: CollectionsState, retry: () -> Unit, open: (LibraryCollection) -> Unit,
    create: (() -> Unit)? = null, edit: ((LibraryCollection) -> Unit)? = null,
    delete: ((LibraryCollection) -> Unit)? = null, move: ((LibraryCollection, LibraryCollection?) -> Unit)? = null) {
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.tab_collections), style = MaterialTheme.typography.headlineLarge, modifier = Modifier.weight(1f))
            create?.let { FilledTonalButton(onClick = it, enabled = !state.busy) { Text(stringResource(R.string.new_collection)) } }
        }
        if (state.busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        state.actionError?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(16.dp)) }
        when {
            state.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            state.error != null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                MemeDockEmptyState(stringResource(R.string.collections_load_failed), failureText(state.error),
                    stringResource(R.string.retry), retry, icon = MemeDockIcons.Folder)
            }
            state.items.isEmpty() -> Column(Modifier.fillMaxSize().padding(24.dp),
                verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
                Icon(MemeDockIcons.Collections, null, Modifier.size(48.dp), tint = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.height(16.dp))
                Text(stringResource(R.string.collections_empty), style = MaterialTheme.typography.titleLarge)
            }
            else -> LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                items(state.items, key = LibraryCollection::id) { collection ->
                    var expanded by remember(collection.id) { mutableStateOf(false) }
                    Surface(onClick = { open(collection) }, shape = MaterialTheme.shapes.large,
                        color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.fillMaxWidth()) {
                        ListItem(headlineContent = { Text(collection.name) },
                            leadingContent = { Icon(MemeDockIcons.Folder, null, tint = MaterialTheme.colorScheme.primary) },
                            trailingContent = { if (edit != null) Box {
                                IconButton(onClick = { expanded = true }, enabled = !state.busy) { Icon(MemeDockIcons.More, stringResource(R.string.organize)) }
                                DropdownMenu(expanded, onDismissRequest = { expanded = false }) {
                                    DropdownMenuItem(text = { Text(stringResource(R.string.rename)) }, onClick = { expanded = false; edit(collection) })
                                    delete?.let { action -> DropdownMenuItem(text = { Text(stringResource(R.string.delete)) }, onClick = { expanded = false; action(collection) }) }
                                    move?.let { action ->
                                        DropdownMenuItem(text = { Text(stringResource(R.string.move_first)) }, onClick = {
                                            expanded = false; action(collection, state.items.firstOrNull { it.id != collection.id })
                                        })
                                        DropdownMenuItem(text = { Text(stringResource(R.string.move_last)) }, onClick = { expanded = false; action(collection, null) })
                                    }
                                }
                            } },
                            colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow))
                    }
                }
            }
        }
    }
}
