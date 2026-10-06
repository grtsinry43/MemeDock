package com.grtsinry43.memedock.feature.detail

import android.content.ActivityNotFoundException
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.platform.clipboard.ClipboardCoordinator
import com.grtsinry43.memedock.platform.saving.SaveCoordinator
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
    fun manage(action: suspend (StickerDetails) -> Unit, success: () -> Unit) {
        val detail = mutable.value.detail ?: return
        if (mutable.value.managing || mutable.value.sharing) return
        mutable.update { it.copy(managing = true, managementError = null) }
        viewModelScope.launch {
            try { action(detail); success() }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(managementError = reason(error)) } }
            finally { mutable.update { it.copy(managing = false) } }
        }
    }
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
            var acquired: OutputLease? = null
            try {
                val lease = repository.export(id, choice, firstFrame)
                acquired = lease
                    val artifact = repository.prepareHandoff(lease)
                    ensureActive()
                    val transferred = deliver(lease, artifact)
                    if (transferred) acquired = null
                    mutable.update { it.copy(shareLaunched = !transferred) }
                    // Once launched, complete accounting despite page disposal.
                    withContext(NonCancellable) { record() }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(shareError = reason(error)) } }
            finally { acquired?.close(); mutable.update { it.copy(sharing = false) } }
        }
    }
    private fun reason(error: Exception) = when (error) {
        is LibraryFailure -> error.reason
        is ActivityNotFoundException -> "NO_SHARE_TARGET"
        is SecurityException -> "PERMISSION_DENIED"
        is java.io.IOException -> "IO"
        else -> "INTERNAL"
    }
}
