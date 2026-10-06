package com.grtsinry43.memedock.feature.detail

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.platform.clipboard.ClipboardCoordinator
import com.grtsinry43.memedock.platform.saving.SaveCoordinator
import com.grtsinry43.memedock.ui.failureCode
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

class DetailViewModel(private val id: String, private val repository: DetailRepository,
    changes: Flow<LibraryChange> = emptyFlow()) : ViewModel() {
    private val mutable = MutableStateFlow(DetailUiState())
    val state = mutable.asStateFlow()
    private var load: Job? = null
    private var sharing: Job? = null
    init {
        retry()
        viewModelScope.launch { changes.filter { it !is LibraryChange.Thumbnail }.collect { retry() } }
    }
    /** Runs [action]; with [refresh] the detail is reloaded before [success], so the page never shows the old value. */
    fun manage(action: suspend (StickerDetails) -> Unit, success: () -> Unit = {}, refresh: Boolean = true) {
        val detail = mutable.value.detail ?: return
        if (mutable.value.managing || mutable.value.sharing) return
        mutable.update { it.copy(managing = true, managementError = null) }
        viewModelScope.launch {
            try {
                action(detail)
                if (refresh) reloadAfterChange()
                success()
            }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(managementError = reason(error)) } }
            finally { mutable.update { it.copy(managing = false) } }
        }
    }
    private suspend fun reloadAfterChange() {
        // The change itself succeeded; a failed reload falls back to the regular retry path.
        try { val fresh = repository.detail(id); mutable.update { it.copy(detail = fresh) } }
        catch (cancel: CancellationException) { throw cancel }
        catch (_: Exception) { retry() }
    }
    fun clearManagementError() { mutable.update { it.copy(managementError = null) } }
    fun retry() {
        load?.cancel()
        mutable.update { it.copy(loading = true, error = null) }
        load = viewModelScope.launch {
            try { val detail = repository.detail(id); mutable.update { it.copy(loading = false, detail = detail) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(loading = false, error = reason(error)) } }
        }
    }
    fun togglePlayback() { mutable.update { it.copy(playing = !it.playing) } }
    fun cancelShare() { sharing?.cancel() }
    fun share(choice: ExportChoice = ExportChoice.Original, firstFrame: Boolean = false, launch: (ShareArtifact) -> Unit) {
        prepare(choice, firstFrame, { _, artifact -> launch(artifact); false }, { repository.recordShareLaunched(id) })
    }
    fun copy(choice: ExportChoice, firstFrame: Boolean, clipboard: ClipboardCoordinator) {
        prepare(choice, firstFrame, { lease, artifact -> clipboard.copy(lease, artifact); mutable.update { it.copy(copied = true) }; false }, { repository.recordCopy(id) })
    }
    fun save(choice: ExportChoice, firstFrame: Boolean, saves: SaveCoordinator) {
        prepare(choice, firstFrame, { lease, artifact -> saves.prepare(id, lease, artifact); true }, {})
    }
    private fun prepare(choice: ExportChoice, firstFrame: Boolean,
        deliver: suspend (OutputLease, ShareArtifact) -> Boolean, record: suspend () -> Unit) {
        if (mutable.value.sharing || mutable.value.managing || mutable.value.detail?.deleted != false) return
        mutable.update { it.copy(sharing = true, shareError = null, shareLaunched = false, copied = false) }
        sharing = viewModelScope.launch {
            try {
                val transferred = repository.output(id, choice, firstFrame, deliver, record)
                mutable.update { it.copy(shareLaunched = !transferred) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(shareError = reason(error)) } }
            finally { mutable.update { it.copy(sharing = false) } }
        }
    }
    private fun reason(error: Exception) = failureCode(error)
}
