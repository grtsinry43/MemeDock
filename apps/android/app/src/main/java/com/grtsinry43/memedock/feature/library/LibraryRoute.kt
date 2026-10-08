package com.grtsinry43.memedock.feature.library

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.feature.detail.StickerRelationsSheet
import com.grtsinry43.memedock.feature.importing.ImportItemStatus
import com.grtsinry43.memedock.feature.importing.ImportPhase
import com.grtsinry43.memedock.feature.importing.ImportState
import com.grtsinry43.memedock.feature.importing.ImportSourceSheet
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureTextRes
import com.grtsinry43.memedock.ui.findActivity

@Composable
fun HomeLibraryRoute(container: AppContainer, contentPadding: PaddingValues, open: (LibraryItem) -> Unit, openTrash: () -> Unit,
    openTelegram: () -> Unit) {
    val model: LibraryViewModel = viewModel(key = "library:home", factory = factory {
        LibraryViewModel(container.library, collections = container.library)
    })
    val state by model.state.collectAsStateWithLifecycle()
    val imports by container.imports.state.collectAsStateWithLifecycle()
    var choosingSource by rememberSaveable { mutableStateOf(false) }
    val photos = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia()) { uris ->
        container.imports.prepare(uris.map { it.toString() })
    }
    val files = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        container.imports.prepare(uris.map { it.toString() })
    }
    val notice = when (imports.phase) {
        ImportPhase.Preparing -> ImportNotice.Preparing(imports.cancellationRequested)
        ImportPhase.Running -> ImportNotice.Running(imports.items.count { it.status in ImportDone }, imports.items.size,
            imports.cancellationRequested)
        ImportPhase.Review -> imports.ready.takeIf { it > 0 }?.let(ImportNotice::Waiting)
        ImportPhase.Finished -> null
    }
    val batchModel = rememberBatchActions(container, "home", state.search to state.filter)
    val selection by batchModel.state.collectAsStateWithLifecycle()
    var batchSheet by remember { mutableStateOf(false) }
    val more = rememberStickerActions(container, openTrash, batchModel::start)
    HomeLibraryScreen(state, container.imageLoader, gridActions(model, open, more, batchModel, selection) { batchSheet = true }, model::search, model::select,
        add = { if (notice is ImportNotice.Waiting) container.imports.show() else choosingSource = true },
        notice = notice, showImports = container.imports::show, stopImports = container.imports::cancel,
        contentPadding = contentPadding)
    BatchActionsSheet(batchSheet, { batchSheet = false }, batchModel, container.library)
    ImportSourceSheet(choosingSource, { choosingSource = false },
        { choosingSource = false; photos.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) },
        { choosingSource = false; files.launch(arrayOf("image/*")) },
        { choosingSource = false; openTelegram() })
}

/** The stickers of one collection or tag. [deleted] runs after the group itself moves to the trash. */
@Composable
fun GroupLibraryRoute(container: AppContainer, group: StickerGroup, name: String, back: () -> Unit,
    open: (LibraryItem) -> Unit, openTrash: () -> Unit, deleted: (StickerGroup) -> Unit) {
    val key = when (group) {
        is StickerGroup.Collection -> "collection:${group.id}"
        is StickerGroup.Tag -> "tag:${group.id}"
    }
    val model: LibraryViewModel = viewModel(key = "library:$key", factory = factory {
        when (group) {
            is StickerGroup.Collection -> LibraryViewModel(container.library, collectionId = group.id)
            is StickerGroup.Tag -> LibraryViewModel(container.library, tagId = group.id)
        }
    })
    val groupModel: GroupViewModel = viewModel(key = "group:$key", factory = factory {
        GroupViewModel(container.library, container.library.changes, group, name)
    })
    val state by model.state.collectAsStateWithLifecycle()
    val groupState by groupModel.state.collectAsStateWithLifecycle()
    var reorderRequested by rememberSaveable(key) { mutableStateOf(false) }
    val reordering = reorderRequested && group is StickerGroup.Collection && state.items.size > 1
    var menu by remember { mutableStateOf(false) }
    val messages = LocalMemeDockMessages.current
    val resources = LocalContext.current.resources
    val removed by rememberUpdatedState(deleted)
    suspend fun fail(reason: String) = messages.showMemeDockMessage(
        MemeDockMessage(resources.getString(failureTextRes(reason)), MemeDockMessageType.Error))
    LaunchedEffect(groupModel) {
        groupModel.events.collect { event ->
            when (event) {
                GroupEvent.Deleted -> removed(group)
                is GroupEvent.Failed -> fail(event.reason)
            }
        }
    }
    LaunchedEffect(model) { model.failures.collect { fail(it) } }
    BackHandler(enabled = reordering) { reorderRequested = false }
    val batchModel = rememberBatchActions(container, key, state.search to state.filter)
    val selection by batchModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(selection.selecting) { if (selection.selecting) reorderRequested = false }
    var batchSheet by remember { mutableStateOf(false) }
    val more = rememberStickerActions(container, openTrash, batchModel::start)
    GroupLibraryScreen(groupState, state, container.imageLoader, gridActions(model, open, more, batchModel, selection) { batchSheet = true }, GroupPageActions(
        back = back,
        more = { menu = true },
        rename = groupModel::rename,
        cancelRename = groupModel::cancelRename,
        finishReorder = { reorderRequested = false },
        reorder = { order, moved -> model.reorder(order, moved) { before -> groupModel.moveItem(moved, before) } },
    ), reordering) {
        when (group) {
            is StickerGroup.Collection -> MemeDockEmptyState(stringResource(R.string.collection_empty),
                stringResource(R.string.collection_empty_hint), icon = MemeDockIcons.Folder)
            is StickerGroup.Tag -> MemeDockEmptyState(stringResource(R.string.tag_empty),
                stringResource(R.string.tag_empty_hint), icon = MemeDockIcons.Label)
        }
    }
    BatchActionsSheet(batchSheet, { batchSheet = false }, batchModel, container.library, collection = groupModel.currentCollection())
    GroupMenuSheet(menu, groupState.name, group, canReorder = !selection.selecting && group is StickerGroup.Collection && state.items.size > 1,
        dismiss = { menu = false }, rename = groupModel::startRename, reorder = { reorderRequested = true },
        delete = groupModel::delete)
}

/** Actions for the page's own collection or tag. Each runs once the sheet is gone, so focus returns to the page first. */
@Composable
private fun GroupMenuSheet(visible: Boolean, name: String, group: StickerGroup, canReorder: Boolean, dismiss: () -> Unit,
    rename: () -> Unit, reorder: () -> Unit, delete: () -> Unit) {
    var pending by remember { mutableStateOf<(() -> Unit)?>(null) }
    MemeDockSheet(
        visible = visible,
        onDismissRequest = { dismiss(); pending?.invoke(); pending = null },
        title = name,
        subtitle = stringResource(if (group is StickerGroup.Collection) R.string.tab_collections else R.string.tags_title),
    ) {
        val sheet = this
        fun then(action: () -> Unit) { pending = action; sheet.dismiss() }
        MemeDockSheetAction(stringResource(R.string.rename), MemeDockIcons.Edit, { then(rename) }, Modifier.testTag("group-rename-action"))
        if (canReorder) MemeDockSheetAction(stringResource(R.string.reorder), MemeDockIcons.DragHandle, { then(reorder) },
            Modifier.testTag("group-reorder"))
        MemeDockSheetAction(stringResource(R.string.delete), MemeDockIcons.Delete, { then(delete) },
            Modifier.testTag("group-delete"), destructive = true)
    }
}

private val ImportDone = ImportState.Unresolved + setOf(ImportItemStatus.Created, ImportItemStatus.Reused)

private fun gridActions(model: LibraryViewModel, open: (LibraryItem) -> Unit, more: (LibraryItem) -> Unit,
    batch: BatchActionsViewModel, selection: BatchSelectionState, organize: () -> Unit) =
    StickerGridActions(model::ensureThumbnail, { if (selection.selecting) batch.toggle(it) else open(it) },
        { if (selection.selecting) batch.toggle(it) else more(it) }, model::loadMore, model::retry,
        selection, { batch.start() }, batch::exit, organize)

/** Hosts the long-press sheet and its follow-ups; returns the callback that opens it for an item. */
@Composable
internal fun rememberStickerActions(container: AppContainer, openTrash: () -> Unit, select: (LibraryItem) -> Unit = {}): (LibraryItem) -> Unit {
    val model: StickerActionsViewModel = viewModel(key = "sticker-actions", factory = factory {
        StickerActionsViewModel(container.library, container.library)
    })
    val state by model.state.collectAsStateWithLifecycle()
    val choice by container.exportPreferences.choice.collectAsStateWithLifecycle(ExportChoice.Original)
    val messages = LocalMemeDockMessages.current
    val context = LocalContext.current
    val trash by rememberUpdatedState(openTrash)
    var target by remember { mutableStateOf<LibraryItem?>(null) }
    LaunchedEffect(model) {
        model.events.collect { event ->
            val resources = context.resources
            when (event) {
                StickerActionEvent.Copied -> messages.showMemeDockMessage(
                    MemeDockMessage(resources.getString(R.string.image_copied), MemeDockMessageType.Success))
                StickerActionEvent.Deleted -> {
                    val result = messages.showMemeDockMessage(MemeDockMessage(resources.getString(R.string.sticker_deleted),
                        MemeDockMessageType.Success, actionLabel = resources.getString(R.string.view),
                        duration = androidx.compose.material3.SnackbarDuration.Long))
                    if (result == SnackbarResult.ActionPerformed) trash()
                }
                is StickerActionEvent.Failed -> messages.showMemeDockMessage(
                    MemeDockMessage(resources.getString(failureTextRes(event.reason)), MemeDockMessageType.Error))
            }
        }
    }
    StickerQuickSheet(
        item = target,
        choice = choice,
        onDismissRequest = { target = null },
        share = { item, selected, firstFrame ->
            context.findActivity()?.let { activity ->
                model.share(item, selected, firstFrame) { container.shares.share(activity, it) }
            }
        },
        copy = { item, selected -> model.copy(item, selected, false, container.clipboard) },
        star = model::toggleStar,
        organize = model::organize,
        delete = model::delete,
        select = select,
    )
    StickerRelationsSheet(state.organizing, container.library, state.busy, state.organizeError, model::closeOrganize, model::saveRelations)
    return { item -> target = item }
}

internal inline fun <reified M : ViewModel> factory(crossinline create: () -> M) = object : ViewModelProvider.Factory {
    override fun <T : ViewModel> create(modelClass: Class<T>): T = modelClass.cast(create())!!
}
