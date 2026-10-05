package com.grtsinry43.memedock.feature.trash

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.*
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.failureText

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TrashRoute(container: AppContainer, back: () -> Unit, open: (LibraryItem) -> Unit) {
    val factory = remember(container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T =
            modelClass.cast(TrashViewModel(container.library, container.library))!!
    } }
    val model: TrashViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        TopAppBar(title = { Text(stringResource(R.string.trash)) },
            navigationIcon = { IconButton(onClick = back) { Icon(MemeDockIcons.Back, stringResource(R.string.back_to_library)) } })
        if (state.loading || state.busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        state.error?.let {
            Text(failureText(it), color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(16.dp))
            TextButton(onClick = model::refresh) { Text(stringResource(R.string.retry)) }
        }
        LazyColumn {
            if (!state.loading && state.items.isEmpty() && state.collections.isEmpty() && state.tags.isEmpty())
                item { Text(stringResource(R.string.trash_empty), Modifier.padding(24.dp)) }
            if (state.items.isNotEmpty()) item { TrashHeading(stringResource(R.string.deleted_stickers)) }
            items(state.items, key = { "sticker:" + it.id }) { item ->
                Surface(onClick = { open(item) }) {
                    ListItem(headlineContent = { Text(item.title) }, supportingContent = { Text(item.originalName) },
                        leadingContent = { Icon(MemeDockIcons.Image, null) },
                        trailingContent = { Text(stringResource(R.string.expand), style = MaterialTheme.typography.labelLarge) })
                }
            }
            if (state.hasMore) item { TextButton(onClick = model::more, enabled = !state.loadingMore, modifier = Modifier.fillMaxWidth()) {
                Text(stringResource(if (state.error != null) R.string.retry else R.string.expand))
            } }
            if (state.collections.isNotEmpty()) item { TrashHeading(stringResource(R.string.deleted_collections)) }
            items(state.collections, key = { "collection:" + it.id }) { value ->
                ListItem(headlineContent = { Text(value.name) }, leadingContent = { Icon(MemeDockIcons.Folder, null) },
                    trailingContent = { TextButton(onClick = { model.restore { container.library.restoreCollection(value) } },
                        enabled = !state.busy) { Text(stringResource(R.string.restore)) } })
            }
            if (state.tags.isNotEmpty()) item { TrashHeading(stringResource(R.string.deleted_tags)) }
            items(state.tags, key = { "tag:" + it.id }) { value ->
                ListItem(headlineContent = { Text(value.name) }, trailingContent = {
                    TextButton(onClick = { model.restore { container.library.restoreTag(value) } },
                        enabled = !state.busy) { Text(stringResource(R.string.restore)) }
                })
            }
        }
    }
}
@Composable
private fun TrashHeading(text: String) {
    Text(text, Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
        style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
}
