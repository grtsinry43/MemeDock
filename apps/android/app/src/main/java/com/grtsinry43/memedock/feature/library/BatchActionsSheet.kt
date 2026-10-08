package com.grtsinry43.memedock.feature.library

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureCode
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
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

private enum class BatchPage { Actions, Collection, AddTags, RemoveTags, Delete }

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun BatchActionsSheet(visible: Boolean, close: () -> Unit, model: BatchActionsViewModel,
    repository: ManagementRepository, trash: Boolean = false, collection: LibraryCollection? = null) {
    val state by model.state.collectAsStateWithLifecycle()
    var page by remember(visible) { mutableStateOf(BatchPage.Actions) }
    var collections by remember { mutableStateOf<List<LibraryCollection>?>(null) }
    var tags by remember { mutableStateOf<List<LibraryTag>?>(null) }
    var loadError by remember(visible) { mutableStateOf<String?>(null) }
    var reload by remember { mutableIntStateOf(0) }
    var chosenCollection by remember(page) { mutableStateOf<String?>(null) }
    var chosenTags by remember(page) { mutableStateOf(emptySet<String>()) }
    var adding by remember(page) { mutableStateOf(false) }
    var creating by remember { mutableStateOf(false) }
    var createError by remember(page) { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(visible, reload) {
        if (visible && !trash) try { loadError = null; collections = repository.collections(false); tags = repository.tags() }
        catch (cancel: CancellationException) { throw cancel }
        catch (failure: Exception) { loadError = failureCode(failure) }
    }
    val locked = state.busy || creating
    val run: (BatchAction) -> Unit = { action -> model.run(action); page = BatchPage.Actions }
    val title = when (page) {
        BatchPage.Actions -> R.string.batch_organize
        BatchPage.Collection -> R.string.assign_collection
        BatchPage.AddTags -> R.string.add_tags
        BatchPage.RemoveTags -> R.string.remove_tags
        BatchPage.Delete -> R.string.delete
    }
    MemeDockSheet(visible, close, title = stringResource(title),
        subtitle = state.selected.size.takeIf { it > 0 }?.let { stringResource(R.string.selection_count, it) },
        dismissible = !locked) {
        val sheet = this
        Column(Modifier.weight(1f, fill = false).imePadding().verticalScroll(rememberScrollState())) {
            when {
                state.busy -> Progress(state)
                page == BatchPage.Actions -> {
                    Outcome(state)
                    if (state.selected.isNotEmpty()) Actions(trash, collection, run) { page = it }
                }
                page == BatchPage.Delete -> MemeDockSheetHint(stringResource(R.string.delete_hint))
                loadError != null -> MemeDockSheetError(failureText(loadError), retry = { reload++ })
                page == BatchPage.Collection -> {
                    val available = collections
                    when {
                        available == null -> MemeDockSheetLoading()
                        available.isEmpty() -> MemeDockSheetHint(stringResource(R.string.no_collections_hint))
                        else -> available.forEach { value ->
                            val checked = value.id == chosenCollection
                            MemeDockSheetChoice(value.name, checked, { chosenCollection = if (checked) null else value.id },
                                Modifier.testTag("batch-collection:${value.id}"), icon = MemeDockIcons.Folder, enabled = !locked)
                        }
                    }
                    if (available != null) MemeDockInlineCreate(
                        label = stringResource(R.string.new_collection),
                        placeholder = stringResource(R.string.name),
                        editing = adding,
                        onStart = { if (!locked) { createError = null; adding = true } },
                        onCreate = { name ->
                            if (!locked) {
                                creating = true
                                createError = null
                                scope.launch {
                                    try {
                                        val created = repository.createCollection(name)
                                        collections = (collections.orEmpty() + created).distinctBy { it.id }
                                        chosenCollection = created.id
                                        adding = false
                                    } catch (cancel: CancellationException) { throw cancel }
                                    catch (failure: Exception) { createError = failureCode(failure) }
                                    finally { creating = false }
                                }
                            }
                        },
                        onCancel = { adding = false; createError = null },
                        modifier = Modifier.testTag("batch-new-collection"),
                        busy = creating,
                        error = createError?.let { failureText(it) },
                    )
                }
                page == BatchPage.AddTags || page == BatchPage.RemoveTags -> {
                    val labels = tags
                    val creatable = page == BatchPage.AddTags
                    when {
                        labels == null -> MemeDockSheetLoading()
                        labels.isEmpty() && !creatable -> MemeDockSheetHint(stringResource(R.string.batch_no_tags))
                        else -> FlowRow(Modifier.fillMaxWidth().padding(horizontal = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            labels.forEach { tag ->
                                MemeDockChoiceChip(tag.name, tag.id in chosenTags, multiple = true, enabled = !locked,
                                    modifier = Modifier.testTag("batch-tag:${tag.id}"), onClick = {
                                        chosenTags = if (tag.id in chosenTags) chosenTags - tag.id else chosenTags + tag.id
                                    })
                            }
                            if (creatable && !adding) MemeDockAddChip(stringResource(R.string.new_tag),
                                { createError = null; adding = true }, Modifier.testTag("batch-new-tag"), enabled = !locked)
                        }
                    }
                    if (labels != null && creatable && adding) MemeDockInlineEdit(
                        initial = "",
                        placeholder = stringResource(R.string.new_tag),
                        onCommit = { name ->
                            creating = true
                            createError = null
                            scope.launch {
                                try {
                                    val tag = repository.createTag(name)
                                    tags = repository.tags()
                                    chosenTags = chosenTags + tag.id
                                    adding = false
                                } catch (cancel: CancellationException) { throw cancel }
                                catch (failure: Exception) { createError = failureCode(failure) }
                                finally { creating = false }
                            }
                        },
                        onCancel = { adding = false; createError = null },
                        modifier = Modifier.padding(horizontal = 24.dp).padding(top = 12.dp),
                        busy = creating,
                        error = createError?.let { failureText(it) },
                    )
                }
            }
        }
        val actionList = !state.busy && page == BatchPage.Actions && state.selected.isNotEmpty()
        if (!actionList) Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(top = 20.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val back = Modifier.testTag("batch-back")
            when {
                state.busy -> MemeDockTextButton(stringResource(R.string.cancel), model::stop, Modifier.testTag("batch-stop"))
                page == BatchPage.Actions -> MemeDockButton(stringResource(R.string.done), { sheet.dismiss() }, Modifier.testTag("batch-done"))
                page == BatchPage.Collection -> {
                    MemeDockButton(stringResource(R.string.assign_collection), {
                        collections.orEmpty().firstOrNull { it.id == chosenCollection }?.let { run(BatchAction.Assign(it)) }
                    }, Modifier.testTag("batch-apply"), enabled = chosenCollection != null && !creating)
                    MemeDockTextButton(stringResource(R.string.back), { page = BatchPage.Actions }, back, enabled = !creating)
                }
                page == BatchPage.AddTags || page == BatchPage.RemoveTags -> {
                    val adds = page == BatchPage.AddTags
                    MemeDockButton(stringResource(if (adds) R.string.add_tags else R.string.remove_tags), {
                        val selected = tags.orEmpty().filter { it.id in chosenTags }
                        run(if (adds) BatchAction.AddTags(selected) else BatchAction.RemoveTags(selected))
                    }, Modifier.testTag("batch-apply"), enabled = chosenTags.isNotEmpty() && !creating)
                    MemeDockTextButton(stringResource(R.string.back), { page = BatchPage.Actions }, back, enabled = !creating)
                }
                page == BatchPage.Delete -> {
                    MemeDockButton(stringResource(R.string.delete), { run(BatchAction.Delete) }, Modifier.testTag("batch-apply"),
                        destructive = true)
                    MemeDockTextButton(stringResource(R.string.cancel), { page = BatchPage.Actions }, back)
                }
            }
        }
    }
}

@Composable
private fun Actions(trash: Boolean, collection: LibraryCollection?, run: (BatchAction) -> Unit, open: (BatchPage) -> Unit) {
    if (trash) {
        MemeDockSheetAction(stringResource(R.string.restore), MemeDockIcons.BackupRestore, { run(BatchAction.Restore) })
        return
    }
    MemeDockSheetAction(stringResource(R.string.assign_collection), MemeDockIcons.Folder, { open(BatchPage.Collection) })
    MemeDockSheetAction(stringResource(if (collection == null) R.string.clear_collection else R.string.remove_from_collection),
        MemeDockIcons.FolderOff, { run(BatchAction.ClearCollection(collection)) })
    MemeDockSheetAction(stringResource(R.string.add_tags), MemeDockIcons.Label, { open(BatchPage.AddTags) })
    MemeDockSheetAction(stringResource(R.string.remove_tags), MemeDockIcons.LabelOff, { open(BatchPage.RemoveTags) })
    MemeDockSheetAction(stringResource(R.string.favorite), MemeDockIcons.Star, { run(BatchAction.Star(true)) })
    MemeDockSheetAction(stringResource(R.string.unfavorite), MemeDockIcons.StarFilled, { run(BatchAction.Star(false)) })
    HorizontalDivider(Modifier.padding(horizontal = 24.dp, vertical = MemeDockLayout.GapXSmall), MemeDockLayout.Hairline,
        MaterialTheme.colorScheme.outlineVariant)
    MemeDockSheetAction(stringResource(R.string.delete), MemeDockIcons.Delete, { open(BatchPage.Delete) }, destructive = true)
}

@Composable
private fun Progress(state: BatchSelectionState) {
    val items = state.report?.items
    val total = items?.size ?: state.selected.size
    val done = items?.count { it.outcome != BatchOutcome.Pending } ?: 0
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        LinearProgressIndicator({ if (total == 0) 0f else done.toFloat() / total }, Modifier.fillMaxWidth())
        Text(stringResource(R.string.batch_progress, done, total), style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Summarises the last run; failed and unprocessed stickers stay selected so the actions below retry them. */
@Composable
private fun Outcome(state: BatchSelectionState) {
    state.report?.let { report ->
        val items = report.items
        MemeDockSheetHint(stringResource(R.string.batch_result,
            items.count { it.outcome == BatchOutcome.Applied || it.outcome == BatchOutcome.Unchanged },
            items.count { it.outcome == BatchOutcome.Failed }, items.count { it.outcome == BatchOutcome.Pending }))
        items.firstNotNullOfOrNull { it.error }?.let { MemeDockSheetHint(failureText(it), error = true) }
    }
    state.error?.let { MemeDockSheetError(failureText(it)) }
    if (state.report != null || state.error != null) Spacer(Modifier.height(MemeDockLayout.GapSmall))
}
