package com.grtsinry43.memedock.feature.tags

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.*
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryTag
import com.grtsinry43.memedock.feature.collections.CollectionEditDialog
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.failureText

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TagsRoute(container: AppContainer, back: () -> Unit) {
    val factory = remember(container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T =
            modelClass.cast(TagsViewModel(container.library, container.library.changes))!!
    } }
    val model: TagsViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    var creating by rememberSaveable { mutableStateOf(false) }
    var editing by remember { mutableStateOf<LibraryTag?>(null) }
    var deleting by remember { mutableStateOf<LibraryTag?>(null) }
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        TopAppBar(title = { Text(stringResource(R.string.manage_tags)) },
            navigationIcon = { IconButton(onClick = back) { Icon(MemeDockIcons.Back, stringResource(R.string.back_to_library)) } },
            actions = { TextButton(onClick = { creating = true }, enabled = !state.busy) { Text(stringResource(R.string.new_tag)) } })
        if (state.loading || state.busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        state.error?.let {
            Text(failureText(it), color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(16.dp))
            TextButton(onClick = model::refresh) { Text(stringResource(R.string.retry)) }
        }
        if (!state.loading && state.items.isEmpty()) Text(stringResource(R.string.tags_empty), Modifier.padding(24.dp))
        LazyColumn {
            items(state.items, key = LibraryTag::id) { tag ->
                var expanded by remember(tag.id) { mutableStateOf(false) }
                ListItem(headlineContent = { Text(tag.name) }, leadingContent = { Icon(MemeDockIcons.Label, null, tint = MaterialTheme.colorScheme.primary) }, trailingContent = {
                    Box {
                        IconButton(onClick = { expanded = true }, enabled = !state.busy) { Icon(MemeDockIcons.More, stringResource(R.string.organize)) }
                        DropdownMenu(expanded, onDismissRequest = { expanded = false }) {
                            DropdownMenuItem(text = { Text(stringResource(R.string.rename)) }, onClick = { expanded = false; editing = tag })
                            DropdownMenuItem(text = { Text(stringResource(R.string.delete)) }, onClick = { expanded = false; deleting = tag })
                        }
                    }
                })
            }
        }
    }
    if (creating || editing != null) key(editing?.id) {
        CollectionEditDialog(stringResource(if (creating) R.string.new_tag else R.string.rename), editing?.name.orEmpty(),
            state.busy, state.error, { creating = false; editing = null }) { name ->
            val tag = editing
            model.manage({ if (tag == null) container.library.createTag(name) else container.library.renameTag(tag, name) },
                { creating = false; editing = null })
        }
    }
    deleting?.let { tag -> AlertDialog(onDismissRequest = { if (!state.busy) deleting = null },
        title = { Text(stringResource(R.string.delete_confirm)) }, text = { Column {
            Text(stringResource(R.string.tag_delete_hint))
            state.error?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error) }
        } },
        dismissButton = { TextButton(onClick = { deleting = null }, enabled = !state.busy) { Text(stringResource(R.string.cancel)) } },
        confirmButton = { TextButton(onClick = { model.manage({ container.library.deleteTag(tag) }, { deleting = null }) },
            enabled = !state.busy) { Text(stringResource(R.string.delete)) } }) }
}
