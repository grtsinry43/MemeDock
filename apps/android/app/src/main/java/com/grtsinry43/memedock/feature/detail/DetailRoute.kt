package com.grtsinry43.memedock.feature.detail

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem

@Composable
fun DetailRoute(id: String, container: AppContainer, initialItem: LibraryItem?, back: () -> Unit) {
    val owner = remember(id) { object : ViewModelStoreOwner { override val viewModelStore = ViewModelStore() } }
    val factory = remember(id, container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T = modelClass.cast(DetailViewModel(id, container.library))!!
    } }
    val model: DetailViewModel = viewModel(viewModelStoreOwner = owner, factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    DisposableEffect(owner) { onDispose { owner.viewModelStore.clear() } }
    DetailScreen(state, container.imageLoader, back, model::retry, model::togglePlayback, model::cancelShare,
        share = {
        model.share { artifact -> container.shares.share(context.activity(), artifact) }
    }, initialItem = initialItem)
}
private fun Context.activity(): Activity = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.activity()
    else -> error("Activity unavailable")
}
