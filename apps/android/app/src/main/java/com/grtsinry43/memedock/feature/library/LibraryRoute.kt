package com.grtsinry43.memedock.feature.library

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import com.grtsinry43.memedock.feature.collections.CollectionOrderSheet
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.feature.importing.ImportItemStatus
import com.grtsinry43.memedock.R
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner

@Composable
fun LibraryRoute(container: AppContainer, searchPage: Boolean = false, collectionId: String? = null,
    title: String = stringResource(if (searchPage) R.string.tab_search else R.string.tab_stickers),
    back: (() -> Unit)? = null, open: (LibraryItem) -> Unit) {
    val owner = if (collectionId == null) androidx.lifecycle.viewmodel.compose.LocalViewModelStoreOwner.current!!
        else remember(collectionId) { object : ViewModelStoreOwner { override val viewModelStore = ViewModelStore() } }
    if (collectionId != null) DisposableEffect(owner) { onDispose { owner.viewModelStore.clear() } }
    val factory = remember(container, collectionId) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T = modelClass.cast(LibraryViewModel(container.library, collectionId))!!
    } }
    val model: LibraryViewModel = viewModel(viewModelStoreOwner = owner, key = "library:$searchPage:$collectionId", factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    val imports by container.imports.state.collectAsStateWithLifecycle()
    var sorting by rememberSaveable(collectionId) { mutableStateOf(false) }
    val photos = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia()) { uris -> container.imports.prepare(uris.map { it.toString() }) }
    val files = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris -> container.imports.prepare(uris.map { it.toString() }) }
    LibraryScreen(state, container.imageLoader, model::search, model::retry, model::loadMore,
        model::ensureThumbnail, { photos.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) },
        { files.launch(arrayOf("image/*")) }, imports.busy, container.imports::show, open,
        waitingForImport = imports.items.any { it.status == ImportItemStatus.Staged || it.status == ImportItemStatus.Queued },
        showSearch = searchPage, title = title, back = back, toggleStarred = model::toggleStarred,
        order = if (collectionId != null) ({ sorting = true }) else null)
    if (sorting && collectionId != null) CollectionOrderSheet(collectionId, container.library, container.library) { sorting = false }
}
