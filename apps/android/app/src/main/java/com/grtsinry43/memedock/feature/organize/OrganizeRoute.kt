package com.grtsinry43.memedock.feature.organize

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryCollection
import com.grtsinry43.memedock.data.library.LibraryTag
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureTextRes

@Composable
fun OrganizeRoute(container: AppContainer, contentPadding: PaddingValues, openCollection: (LibraryCollection) -> Unit,
    openTag: (LibraryTag) -> Unit, openTrash: () -> Unit, viewAll: () -> Unit = {}, allCollections: Boolean = false, back: () -> Unit = {}) {
    val factory = remember(container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T =
            modelClass.cast(OrganizeViewModel(container.library, container.library.changes))!!
    } }
    val model: OrganizeViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    var reorderRequested by rememberSaveable { mutableStateOf(false) }
    val reordering = reorderRequested && state.collections.size > 1
    var target by remember { mutableStateOf<OrganizeItem?>(null) }
    val messages = LocalMemeDockMessages.current
    val resources = LocalContext.current.resources
    val trash by rememberUpdatedState(openTrash)
    LaunchedEffect(model) {
        model.events.collect { event ->
            when (event) {
                is OrganizeEvent.Deleted -> {
                    val text = resources.getString(if (event.tag) R.string.tag_deleted else R.string.collection_deleted)
                    val result = messages.showMemeDockMessage(MemeDockMessage(text, MemeDockMessageType.Success,
                        actionLabel = resources.getString(R.string.view), duration = SnackbarDuration.Long))
                    if (result == SnackbarResult.ActionPerformed) trash()
                }
                is OrganizeEvent.Failed -> messages.showMemeDockMessage(
                    MemeDockMessage(resources.getString(failureTextRes(event.reason)), MemeDockMessageType.Error))
            }
        }
    }
    LaunchedEffect(state.loading, state.collections.size) {
        if (!state.loading && state.collections.size < 2) reorderRequested = false
    }
    BackHandler(enabled = reordering) { reorderRequested = false }
    OrganizeScreen(state, reordering, OrganizeActions(
        retry = model::retry,
        openCollection = openCollection,
        openTag = openTag,
        more = { target = it },
        edit = model::edit,
        commit = model::commit,
        move = model::moveCollection,
        finishReorder = { reorderRequested = false },
        viewAll = viewAll,
    ), contentPadding, container.imageLoader, container.library::thumbnail, allCollections, back)
    OrganizeItemSheet(target, canReorder = state.collections.size > 1, dismiss = { target = null },
        rename = model::edit, reorder = { reorderRequested = true }, delete = model::delete)
}

/** Actions for a long-pressed collection or tag. Each runs once the sheet is gone, so focus returns to the page first. */
@Composable
private fun OrganizeItemSheet(target: OrganizeItem?, canReorder: Boolean, dismiss: () -> Unit, rename: (OrganizeItem) -> Unit,
    reorder: () -> Unit, delete: (OrganizeItem) -> Unit) {
    val shown = rememberRetained(target) ?: return
    var pending by remember { mutableStateOf<(() -> Unit)?>(null) }
    val collection = shown is OrganizeItem.Collection
    MemeDockSheet(
        visible = target != null,
        onDismissRequest = { dismiss(); pending?.invoke(); pending = null },
        title = shown.name,
        subtitle = stringResource(if (collection) R.string.tab_collections else R.string.tags_title),
    ) {
        val sheet = this
        fun then(action: () -> Unit) { pending = action; sheet.dismiss() }
        MemeDockSheetAction(stringResource(R.string.rename), MemeDockIcons.Edit, { then { rename(shown) } },
            Modifier.testTag("organize-rename"))
        if (collection && canReorder) MemeDockSheetAction(stringResource(R.string.reorder), MemeDockIcons.DragHandle,
            { then(reorder) }, Modifier.testTag("organize-reorder"))
        MemeDockSheetAction(stringResource(R.string.delete), MemeDockIcons.Delete, { then { delete(shown) } },
            Modifier.testTag("organize-delete"), destructive = true)
    }
}
