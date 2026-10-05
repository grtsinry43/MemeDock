package com.grtsinry43.memedock.feature.library

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.*
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.feature.importing.ImportSheet

@Composable
fun LibraryRoute(container: AppContainer) {
    val factory = remember(container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T = modelClass.cast(LibraryViewModel(container.library))!!
    } }
    val model: LibraryViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    val imports by container.imports.state.collectAsStateWithLifecycle()
    val photos = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia()) { uris -> container.imports.prepare(uris.map { it.toString() }) }
    val files = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris -> container.imports.prepare(uris.map { it.toString() }) }
    LibraryScreen(state, container.imageLoader, model::search, model::retry, model::loadMore,
        model::ensureThumbnail, { photos.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) },
        { files.launch(arrayOf("image/*")) }, imports.busy, container.imports::show)
    if (imports.visible) ImportSheet(imports, container.imports)
}
