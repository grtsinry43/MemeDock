package com.grtsinry43.memedock.feature.detail

import android.content.ActivityNotFoundException
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
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
    fun share(launch: (ShareArtifact) -> Unit) {
        if (mutable.value.sharing || mutable.value.managing || mutable.value.detail?.deleted != false) return
        mutable.update { it.copy(sharing = true, shareError = null, shareLaunched = false) }
        sharing = viewModelScope.launch {
            try {
                repository.exportOriginal(id).use { lease ->
                    val artifact = repository.prepareHandoff(lease)
                    ensureActive()
                    launch(artifact)
                    mutable.update { it.copy(shareLaunched = true) }
                    // Once launched, complete accounting despite page disposal.
                    withContext(NonCancellable) { repository.recordShareLaunched(id) }
                }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(shareError = reason(error)) } }
            finally { mutable.update { it.copy(sharing = false) } }
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
