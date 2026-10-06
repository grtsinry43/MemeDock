package com.grtsinry43.memedock.feature.detail

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.material3.*
import androidx.compose.foundation.layout.Column
import com.grtsinry43.memedock.ui.failureText
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.feature.trash.RestoreSheet
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.settings.ExportChoice
import kotlinx.coroutines.launch

private enum class OutputAction { Share, Copy, Save }

@Composable
fun DetailRoute(id: String, container: AppContainer, initialItem: LibraryItem?, back: () -> Unit) {
    val owner = remember(id) { object : ViewModelStoreOwner { override val viewModelStore = ViewModelStore() } }
    val factory = remember(id, container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T = modelClass.cast(DetailViewModel(id, container.library, container.library.changes))!!
    } }
    val model: DetailViewModel = viewModel(viewModelStoreOwner = owner, factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val preference by container.exportPreferences.choice.collectAsStateWithLifecycle<ExportChoice?>(null)
    val choice = preference ?: ExportChoice.Original
    val saveState by container.saves.state.collectAsStateWithLifecycle()
    var choosingPreset by rememberSaveable(id) { mutableStateOf(false) }
    var pendingAction by rememberSaveable(id) { mutableStateOf<OutputAction?>(null) }
    var pendingChoice by rememberSaveable(id) { mutableStateOf(ExportChoice.Original) }
    var preferenceError by remember { mutableStateOf<String?>(null) }
    fun perform(action: OutputAction, selected: ExportChoice, firstFrame: Boolean) {
        when (action) {
            OutputAction.Share -> model.share(selected, firstFrame) { container.shares.share(context.activity(), it) }
            OutputAction.Copy -> model.copy(selected, firstFrame, container.clipboard)
            OutputAction.Save -> model.save(selected, firstFrame, container.saves)
        }
    }
    fun request(action: OutputAction) {
        if (preference == null || saveState.artifact != null || state.sharing) return
        container.saves.clearFeedback()
        if (state.detail?.animated == true && choice != ExportChoice.Original) {
            pendingChoice = choice; pendingAction = action
        } else perform(action, choice, false)
    }
    var editing by rememberSaveable(id) { mutableStateOf(false) }
    var organizing by rememberSaveable(id) { mutableStateOf(false) }
    var deleting by rememberSaveable(id) { mutableStateOf(false) }
    var restoring by rememberSaveable(id) { mutableStateOf(false) }
    DisposableEffect(owner) { onDispose { owner.viewModelStore.clear() } }
    val shown = state.copy(sharing = state.sharing || saveState.artifact != null,
        saving = saveState.writing, saved = saveState.stickerId == id && saveState.saved,
        shareError = preferenceError ?: if (saveState.stickerId == id) saveState.error ?: state.shareError else state.shareError)
    DetailScreen(shown, container.imageLoader, back, model::retry, model::togglePlayback,
        { if (saveState.writing) container.saves.cancel() else model.cancelShare() },
        share = {
        request(OutputAction.Share)
    }, initialItem = initialItem,
        edit = { editing = true }, organize = { organizing = true },
        favorite = { model.manage({ container.library.patchSticker(it, starred = !it.starred) }, model::retry) },
        delete = { deleting = true }, restore = { restoring = true }, choice = choice,
        choosePreset = { choosingPreset = true }, copy = { request(OutputAction.Copy) }, save = { request(OutputAction.Save) },
        outputsReady = preference != null)
    if (choosingPreset) ExportPresetSheet(choice, { choosingPreset = false }) { selected ->
        scope.launch {
            try { container.exportPreferences.select(selected); preferenceError = null; choosingPreset = false }
            catch (cancel: kotlinx.coroutines.CancellationException) { throw cancel }
            catch (_: java.io.IOException) { preferenceError = "IO" }
        }
    }
    pendingAction?.let { action -> AlertDialog(onDismissRequest = { pendingAction = null },
        title = { Text(stringResource(R.string.export_animation_title)) },
        text = { Text(stringResource(R.string.export_animation_hint)) },
        dismissButton = { TextButton(onClick = { pendingAction = null }) { Text(stringResource(R.string.cancel)) } },
        confirmButton = { TextButton(onClick = { pendingAction = null; perform(action, pendingChoice, true) },
            modifier = Modifier.testTag("confirm-first-frame")) { Text(stringResource(R.string.export_first_frame)) } }) }
    state.detail?.let { detail ->
        if (restoring) {
            val observed = remember { detail }
            RestoreSheet(id, container.library, state.managing, state.managementError, { restoring = false }) {
            model.manage({ container.library.restoreSticker(observed) }, { restoring = false; model.retry() })
            }
        }
        if (editing) {
            val observed = remember { detail }
            StickerEditSheet(observed, state.managing, state.managementError, { editing = false }) { title, note ->
            model.manage({ container.library.patchSticker(observed, title, note) }, { editing = false; model.retry() })
            }
        }
        if (organizing) {
            val observed = remember { detail }
            StickerRelationsSheet(observed, container.library, state.managing, state.managementError, { organizing = false }) { collections, tags ->
            model.manage({ container.library.relations(observed, collections, tags) }, { organizing = false; model.retry() })
            }
        }
        if (deleting) AlertDialog(onDismissRequest = { if (!state.managing) deleting = false },
            title = { Text(stringResource(R.string.delete_confirm)) }, text = { Column {
                Text(stringResource(R.string.delete_hint))
                state.managementError?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error) }
            } },
            dismissButton = { TextButton(onClick = { deleting = false }, enabled = !state.managing) { Text(stringResource(R.string.cancel)) } },
            confirmButton = { TextButton(onClick = {
                model.manage(container.library::deleteSticker, { deleting = false; back() })
            }, enabled = !state.managing, modifier = Modifier.testTag("confirm-delete-sticker")) { Text(stringResource(R.string.delete)) } })
    }
}
private fun Context.activity(): Activity = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.activity()
    else -> error("Activity unavailable")
}
