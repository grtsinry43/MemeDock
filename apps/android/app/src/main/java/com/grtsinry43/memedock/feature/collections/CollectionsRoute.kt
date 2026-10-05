package com.grtsinry43.memedock.feature.collections

import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.material3.*
import androidx.compose.foundation.layout.Column
import com.grtsinry43.memedock.ui.failureText
import androidx.compose.ui.res.stringResource
import com.grtsinry43.memedock.R
import androidx.lifecycle.*
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryCollection

@Composable
fun CollectionsRoute(container: AppContainer, open: (LibraryCollection) -> Unit) {
    val factory = remember(container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T =
            modelClass.cast(CollectionsViewModel(container.library, container.library.changes))!!
    } }
    val model: CollectionsViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    var creating by rememberSaveable { mutableStateOf(false) }
    var editing by remember { mutableStateOf<LibraryCollection?>(null) }
    var deleting by remember { mutableStateOf<LibraryCollection?>(null) }
    CollectionsScreen(state, model::retry, open, { creating = true }, { editing = it }, { deleting = it },
        { value, before -> model.manage({ container.library.moveCollection(value, before) }, {}) })
    if (creating || editing != null) key(editing?.id) {
        CollectionEditDialog(stringResource(if (creating) R.string.new_collection else R.string.rename), editing?.name.orEmpty(),
            state.busy, state.actionError, { creating = false; editing = null }) { name ->
            val value = editing
            model.manage({ if (value == null) container.library.createCollection(name) else container.library.renameCollection(value, name) },
                { creating = false; editing = null })
        }
    }
    deleting?.let { value -> AlertDialog(onDismissRequest = { if (!state.busy) deleting = null },
        title = { Text(stringResource(R.string.delete_confirm)) }, text = { Column {
            Text(stringResource(R.string.collection_delete_hint))
            state.actionError?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error) }
        } },
        dismissButton = { TextButton(onClick = { deleting = null }, enabled = !state.busy) { Text(stringResource(R.string.cancel)) } },
        confirmButton = { TextButton(onClick = { model.manage({ container.library.deleteCollection(value) }, { deleting = null }) },
            enabled = !state.busy) { Text(stringResource(R.string.delete)) } }) }
}
