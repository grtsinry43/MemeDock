package com.grtsinry43.memedock.feature.organize

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText

@Composable
internal fun CollectionsScreen(state: OrganizeState, actions: OrganizeActions, loader: ImageLoader,
    thumbnail: suspend (String) -> String, back: () -> Unit) {
    val summaries = remember(state.summaries) { state.summaries.associateBy { it.collection.id } }
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).windowInsetsPadding(WindowInsets.safeDrawing)) {
        Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = back) { Icon(MemeDockIcons.Back, stringResource(R.string.back)) }
            Text(stringResource(R.string.tab_collections), Modifier.weight(1f), style = MaterialTheme.typography.headlineSmall)
            TextButton(onClick = { actions.edit(OrganizeEdit.NewCollection) }) { Text(stringResource(R.string.new_collection)) }
        }
        if (state.loading) CircularProgressIndicator(Modifier.padding(24.dp))
        else if (state.error != null) Column(Modifier.padding(24.dp)) {
            Text(failureText(state.error), color = MaterialTheme.colorScheme.error)
            TextButton(onClick = actions.retry) { Text(stringResource(R.string.retry)) }
        }
        CollectionEditor(state, actions)
        LazyVerticalGrid(GridCells.Adaptive(152.dp), Modifier.weight(1f), contentPadding = PaddingValues(12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            items(state.collections, key = { it.id }) { collection ->
                val summary = summaries[collection.id] ?: return@items
                CollectionCard(summary, loader, thumbnail, { actions.openCollection(collection) }, { actions.more(OrganizeItem.Collection(collection)) })
            }
        }
    }
}

@Composable
internal fun CollectionEditor(state: OrganizeState, actions: OrganizeActions) {
    val editing = state.editing
    if (editing == OrganizeEdit.NewCollection || editing is OrganizeItem.Collection) MemeDockInlineEdit(
        (editing as? OrganizeItem.Collection)?.name.orEmpty(), stringResource(R.string.name), actions.commit, { actions.edit(null) },
        Modifier.padding(horizontal = 16.dp, vertical = 8.dp), busy = state.busy, error = state.editError?.let { failureText(it) })
}
