package com.grtsinry43.memedock.feature.library

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

@Composable
internal fun rememberBatchActions(container: AppContainer, key: String, query: Any): BatchActionsViewModel {
    val model: BatchActionsViewModel = viewModel(key = "batch:$key", factory = factory { BatchActionsViewModel(container.library) })
    val state by model.state.collectAsStateWithLifecycle()
    LaunchedEffect(query) { model.leave() }
    DisposableEffect(model) { onDispose { model.leave() } }
    BackHandler(state.selecting && !state.busy) { model.exit() }
    return model
}

@Composable
internal fun BatchActionsSheet(visible: Boolean, close: () -> Unit, model: BatchActionsViewModel,
    repository: ManagementRepository, trash: Boolean = false, collection: LibraryCollection? = null) {
    val state by model.state.collectAsStateWithLifecycle()
    var mode by remember(visible) { mutableIntStateOf(0) }
    var collections by remember { mutableStateOf<List<LibraryCollection>?>(null) }
    var tags by remember { mutableStateOf<List<LibraryTag>?>(null) }
    var chosenTags by remember(mode) { mutableStateOf(emptySet<String>()) }
    var error by remember(visible) { mutableStateOf<String?>(null) }
    var addingCollection by remember(visible) { mutableStateOf(false) }
    var creating by remember { mutableStateOf(false) }
    var createError by remember(visible) { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    var retry by remember { mutableIntStateOf(0) }
    LaunchedEffect(visible, retry) {
        if (visible) try { error = null; collections = repository.collections(false); tags = repository.tags() }
        catch (cancel: CancellationException) { throw cancel }
        catch (failure: Exception) { error = com.grtsinry43.memedock.ui.failureCode(failure) }
    }
    MemeDockSheet(visible, close, title = stringResource(R.string.batch_organize), dismissible = !state.busy && !creating) {
        Column(Modifier.weight(1f, fill = false).imePadding().verticalScroll(rememberScrollState())) {
            Text(stringResource(R.string.selection_count, state.selected.size), Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            if (state.busy) {
                val done = state.report?.items?.count { it.outcome != BatchOutcome.Pending } ?: 0
                LinearProgressIndicator(Modifier.fillMaxWidth().padding(24.dp))
                Text(stringResource(R.string.batch_progress, done, state.report?.items?.size ?: state.selected.size), Modifier.padding(horizontal = 24.dp))
                TextButton(onClick = model::stop, modifier = Modifier.padding(horizontal = 16.dp)) { Text(stringResource(R.string.cancel)) }
            } else {
                state.report?.let { report ->
                    Text(stringResource(R.string.batch_result, report.items.count { it.outcome in setOf(BatchOutcome.Applied, BatchOutcome.Unchanged) },
                        report.items.count { it.outcome == BatchOutcome.Failed }, report.items.count { it.outcome == BatchOutcome.Pending }), Modifier.padding(24.dp))
                    report.items.firstOrNull { it.error != null }?.error?.let { Text(failureText(it), Modifier.padding(horizontal = 24.dp), color = MaterialTheme.colorScheme.error) }
                }
                (state.error ?: error)?.let { Text(failureText(it), Modifier.padding(24.dp), color = MaterialTheme.colorScheme.error) }
                if (error != null) TextButton(onClick = { retry++ }) { Text(stringResource(R.string.retry)) }
                if (state.selected.isNotEmpty()) when (mode) {
                    0 -> {
                        if (trash) MemeDockSheetAction(stringResource(R.string.restore), MemeDockIcons.BackupRestore, { model.run(BatchAction.Restore) })
                        else {
                            MemeDockSheetAction(stringResource(R.string.assign_collection), MemeDockIcons.Collections, { mode = 1 })
                            MemeDockSheetAction(stringResource(R.string.add_tags), MemeDockIcons.Label, { mode = 2 })
                            MemeDockSheetAction(stringResource(R.string.remove_tags), MemeDockIcons.Label, { mode = 3 })
                            MemeDockSheetAction(stringResource(R.string.favorite), MemeDockIcons.Star, { model.run(BatchAction.Star(true)) })
                            MemeDockSheetAction(stringResource(R.string.unfavorite), MemeDockIcons.Star, { model.run(BatchAction.Star(false)) })
                            MemeDockSheetAction(stringResource(if (collection == null) R.string.clear_collection else R.string.remove_from_collection), MemeDockIcons.Folder,
                                { model.run(BatchAction.ClearCollection(collection)) })
                            MemeDockSheetAction(stringResource(R.string.delete), MemeDockIcons.Delete, { mode = 4 }, destructive = true)
                        }
                    }
                    1 -> {
                        TextButton(onClick = { mode = 0 }, enabled = !creating && !addingCollection) { Text(stringResource(R.string.back)) }
                        if (collections == null) CircularProgressIndicator(Modifier.padding(24.dp))
                        collections?.forEach { value -> MemeDockSheetAction(value.name, MemeDockIcons.Folder, { model.run(BatchAction.Assign(value)); mode = 0 }, enabled = !creating && !addingCollection) }
                        if (collections?.isEmpty() == true) Text(stringResource(R.string.no_collections_hint), Modifier.padding(24.dp))
                        if (collections != null) MemeDockInlineCreate(
                            label = stringResource(R.string.new_collection),
                            placeholder = stringResource(R.string.name),
                            editing = addingCollection,
                            onStart = { createError = null; addingCollection = true },
                            onCreate = { name ->
                                if (!creating) {
                                    creating = true
                                    createError = null
                                    scope.launch {
                                        try {
                                            val created = repository.createCollection(name)
                                            collections = (collections.orEmpty() + created).distinctBy { it.id }
                                            addingCollection = false
                                            model.run(BatchAction.Assign(created))
                                            mode = 0
                                        } catch (cancel: CancellationException) { throw cancel }
                                        catch (failure: Exception) { createError = com.grtsinry43.memedock.ui.failureCode(failure) }
                                        finally { creating = false }
                                    }
                                }
                            },
                            onCancel = { addingCollection = false; createError = null },
                            busy = creating,
                            error = createError?.let { failureText(it) },
                            modifier = Modifier.testTag("batch-new-collection"),
                        )
                    }
                    2, 3 -> {
                        TextButton(onClick = { mode = 0 }) { Text(stringResource(R.string.back)) }
                        if (tags == null) CircularProgressIndicator(Modifier.padding(24.dp))
                        tags?.forEach { tag ->
                            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                                Checkbox(tag.id in chosenTags, { chosenTags = if (it) chosenTags + tag.id else chosenTags - tag.id })
                                Text(tag.name)
                            }
                        }
                        Button(onClick = { val selected = tags.orEmpty().filter { it.id in chosenTags }; model.run(if (mode == 2) BatchAction.AddTags(selected) else BatchAction.RemoveTags(selected)); mode = 0 },
                            enabled = chosenTags.isNotEmpty(), modifier = Modifier.padding(24.dp)) { Text(stringResource(R.string.done)) }
                    }
                    4 -> {
                        Text(stringResource(R.string.delete_hint), Modifier.padding(24.dp))
                        TextButton(onClick = { mode = 0 }) { Text(stringResource(R.string.cancel)) }
                        Button(onClick = { model.run(BatchAction.Delete); mode = 0 }, modifier = Modifier.padding(24.dp)) { Text(stringResource(R.string.delete)) }
                    }
                }
            }
        }
    }
}
