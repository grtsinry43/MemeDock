package com.grtsinry43.memedock.feature.collections

import androidx.compose.runtime.*
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
    CollectionsScreen(state, model::retry, open)
}
