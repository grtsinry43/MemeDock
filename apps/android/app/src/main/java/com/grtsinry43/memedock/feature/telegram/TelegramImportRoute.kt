package com.grtsinry43.memedock.feature.telegram

import androidx.activity.compose.BackHandler
import androidx.compose.runtime.*
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.ui.components.MemeDockConfirmSheet

@Composable
fun TelegramImportRoute(container: AppContainer, back: () -> Unit) {
    val factory = remember(container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T =
            modelClass.cast(TelegramImportViewModel(container.telegram, container.telegramTokens))!!
    } }
    val model: TelegramImportViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    var leaving by remember { mutableStateOf(false) }
    val requestBack = { if (state.importing || state.loading) leaving = true else back() }
    BackHandler(state.importing || state.loading) { leaving = true }
    TelegramImportScreen(state, model, container.imageLoader, requestBack)
    MemeDockConfirmSheet(visible = leaving, title = stringResource(R.string.telegram_leave_title),
        message = stringResource(R.string.telegram_leave_hint), confirmLabel = stringResource(R.string.telegram_leave),
        onConfirm = { leaving = false; model.cancel(); back() }, onDismissRequest = { leaving = false })
}
