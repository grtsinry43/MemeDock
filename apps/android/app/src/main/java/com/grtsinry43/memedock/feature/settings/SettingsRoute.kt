package com.grtsinry43.memedock.feature.settings

import androidx.compose.runtime.*
import androidx.lifecycle.*
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.app.AppContainer

@Composable
fun SettingsRoute(container: AppContainer) {
    val factory = remember(container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T =
            modelClass.cast(SettingsViewModel(container.appearance, container.library))!!
    } }
    val model: SettingsViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    LaunchedEffect(model) { model.refresh() }
    SettingsScreen(state, model::select, model::retry)
}
