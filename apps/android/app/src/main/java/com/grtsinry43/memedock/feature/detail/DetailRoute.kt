package com.grtsinry43.memedock.feature.detail

import android.os.Build
import androidx.annotation.StringRes
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
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
import com.grtsinry43.memedock.feature.trash.RestoreSheet
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureTextRes
import com.grtsinry43.memedock.ui.findActivity
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

private enum class OutputAction { Share, Copy, Save }

/** [deleted] leaves the page; the caller owns the follow-up message because this route is gone by then. */
@Composable
fun DetailRoute(id: String, container: AppContainer, initialItem: LibraryItem?, back: () -> Unit, deleted: () -> Unit) {
    val factory = remember(id, container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T = modelClass.cast(DetailViewModel(id, container.library, container.library.changes))!!
    } }
    val model: DetailViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val resources = LocalResources.current
    val scope = rememberCoroutineScope()
    val messages = LocalMemeDockMessages.current
    val preference by container.exportPreferences.choice.collectAsStateWithLifecycle<ExportChoice?>(null)
    val choice = preference ?: ExportChoice.Original
    val saveState by container.saves.state.collectAsStateWithLifecycle()
    var choosingPreset by rememberSaveable(id) { mutableStateOf(false) }
    var more by rememberSaveable(id) { mutableStateOf(false) }
    var organizing by rememberSaveable(id) { mutableStateOf(false) }
    var restoring by rememberSaveable(id) { mutableStateOf(false) }
    var editing by rememberSaveable(id) { mutableStateOf<DetailField?>(null) }

    // Messages outlive the effect that raised them, so they run in the route scope.
    fun notify(@StringRes text: Int, type: MemeDockMessageType) {
        val message = MemeDockMessage(resources.getString(text), type)
        scope.launch { messages.showMemeDockMessage(message) }
    }
    fun request(action: OutputAction, firstFrame: Boolean) {
        if (preference == null || saveState.artifact != null || state.sharing) return
        container.saves.clearFeedback()
        // Animated stickers keep their motion unless the first frame is asked for explicitly.
        val selected = if (state.detail?.animated == true && !firstFrame) ExportChoice.Original else choice
        when (action) {
            OutputAction.Share -> context.findActivity()?.let { activity ->
                model.share(selected, firstFrame) { container.shares.share(activity, it) }
            }
            OutputAction.Copy -> model.copy(selected, firstFrame, container.clipboard)
            OutputAction.Save -> model.save(selected, firstFrame, container.saves)
        }
    }

    // Android 13+ confirms copies itself.
    LaunchedEffect(state.copied) {
        if (state.copied && Build.VERSION.SDK_INT < 33) notify(R.string.image_copied, MemeDockMessageType.Success)
    }
    LaunchedEffect(state.shareError) {
        val code = state.shareError ?: return@LaunchedEffect
        if (state.shareLaunched) notify(R.string.output_accounting_failed, MemeDockMessageType.Warning)
        else notify(failureTextRes(code), MemeDockMessageType.Error)
    }
    val saveResult = saveState.takeIf { it.stickerId == id && it.artifact == null && !it.writing && (it.saved || it.error != null) }
    LaunchedEffect(saveResult) {
        val result = saveResult ?: return@LaunchedEffect
        container.saves.clearFeedback()
        val error = result.error
        when {
            result.saved && error != null -> notify(R.string.output_accounting_failed, MemeDockMessageType.Warning)
            result.saved -> notify(R.string.image_saved, MemeDockMessageType.Success)
            error != null -> notify(failureTextRes(error), MemeDockMessageType.Error)
        }
    }
    // Failures of inline edits and sheets are shown where they happened; the rest become messages.
    LaunchedEffect(state.managementError, editing, organizing, restoring) {
        val code = state.managementError ?: return@LaunchedEffect
        if (editing == null && !organizing && !restoring) {
            notify(failureTextRes(code), MemeDockMessageType.Error)
            model.clearManagementError()
        }
    }

    val shown = state.copy(sharing = state.sharing || saveState.artifact != null, saving = saveState.writing)
    DetailScreen(
        state = shown,
        loader = container.imageLoader,
        initialItem = initialItem,
        choice = choice,
        outputsReady = preference != null,
        editing = editing,
        actions = DetailActions(
            back = back,
            retry = model::retry,
            togglePlayback = model::togglePlayback,
            share = { request(OutputAction.Share, it) },
            copy = { request(OutputAction.Copy, false) },
            save = { request(OutputAction.Save, false) },
            cancelOutput = { if (saveState.writing) container.saves.cancel() else model.cancelShare() },
            choosePreset = { choosingPreset = true },
            star = { model.manage({ container.library.patchSticker(it, starred = !it.starred) }) },
            organize = { organizing = true },
            more = { more = true },
            restore = { restoring = true },
            edit = { field -> editing = field; if (field == null) model.clearManagementError() },
            commit = { field, text ->
                model.manage({
                    when (field) {
                        DetailField.Title -> container.library.patchSticker(it, title = text)
                        DetailField.Note -> container.library.patchSticker(it, note = text)
                    }
                }, { editing = null })
            },
        ),
    )

    ExportPresetSheet(choosingPreset, choice, { choosingPreset = false }) { selected ->
        scope.launch {
            try { container.exportPreferences.select(selected); choosingPreset = false }
            catch (cancel: CancellationException) { throw cancel }
            catch (_: java.io.IOException) { notify(failureTextRes("IO"), MemeDockMessageType.Error) }
        }
    }
    val detail = rememberRetained(state.detail)
    MemeDockSheet(more && state.detail?.deleted == false, { more = false }, title = detail?.title) {
        MemeDockSheetAction(stringResource(R.string.organize), MemeDockIcons.Label,
            { more = false; organizing = true }, supporting = stringResource(R.string.organize_hint))
        MemeDockSheetAction(stringResource(R.string.delete), MemeDockIcons.Delete, {
            more = false
            model.manage(container.library::deleteSticker, deleted, refresh = false)
        }, modifier = Modifier.testTag("delete-sticker"), supporting = stringResource(R.string.delete_hint), destructive = true)
    }
    StickerRelationsSheet(state.detail?.takeIf { organizing && !it.deleted }, container.library, state.managing, state.managementError,
        close = { organizing = false; model.clearManagementError() }) { collections, tags ->
        model.manage({ container.library.relations(it, collections, tags) }, { organizing = false })
    }
    RestoreSheet(restoring && state.detail?.deleted == true, id, container.library, state.managing, state.managementError,
        { restoring = false; model.clearManagementError() }) {
        model.manage(container.library::restoreSticker, { restoring = false })
    }
}
