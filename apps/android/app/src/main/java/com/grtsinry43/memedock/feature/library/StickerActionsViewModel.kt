package com.grtsinry43.memedock.feature.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.platform.clipboard.ClipboardCoordinator
import com.grtsinry43.memedock.ui.failureCode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch

sealed interface StickerActionEvent {
    data object Copied : StickerActionEvent
    data object Deleted : StickerActionEvent
    data class Failed(val reason: String) : StickerActionEvent
}

data class StickerActionsState(val busy: Boolean = false, val organizing: StickerDetails? = null, val organizeError: String? = null)

/** Grid quick actions. One action runs at a time; results arrive as one-shot [events]. */
class StickerActionsViewModel(private val details: DetailRepository, private val management: ManagementRepository) : ViewModel() {
    private val mutable = MutableStateFlow(StickerActionsState())
    val state = mutable.asStateFlow()
    private val channel = Channel<StickerActionEvent>(Channel.BUFFERED)
    val events = channel.receiveAsFlow()

    fun share(item: LibraryItem, choice: ExportChoice, firstFrame: Boolean, launch: (ShareArtifact) -> Unit) = perform {
        details.output(item.id, choice, firstFrame, { _, artifact -> launch(artifact); false }) { details.recordShareLaunched(item.id) }
    }

    fun copy(item: LibraryItem, choice: ExportChoice, firstFrame: Boolean, clipboard: ClipboardCoordinator) = perform(StickerActionEvent.Copied) {
        details.output(item.id, choice, firstFrame, { lease, artifact -> clipboard.copy(lease, artifact); false }) { details.recordCopy(item.id) }
    }

    fun toggleStar(item: LibraryItem) = perform { management.patchSticker(details.detail(item.id), starred = !item.starred) }

    fun delete(item: LibraryItem) = perform(StickerActionEvent.Deleted) { management.deleteSticker(details.detail(item.id)) }

    fun organize(item: LibraryItem) = perform { mutable.update { it.copy(organizing = details.detail(item.id), organizeError = null) } }

    fun saveRelations(collection: LibraryCollection?, tags: List<LibraryTag>) {
        val detail = mutable.value.organizing ?: return
        if (mutable.value.busy) return
        mutable.update { it.copy(busy = true, organizeError = null) }
        viewModelScope.launch {
            try { management.relations(detail, collection, tags); mutable.update { it.copy(organizing = null) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(organizeError = failureCode(error)) } }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }

    fun closeOrganize() { if (!mutable.value.busy) mutable.update { it.copy(organizing = null, organizeError = null) } }

    private fun perform(success: StickerActionEvent? = null, action: suspend () -> Unit) {
        if (mutable.value.busy) return
        mutable.update { it.copy(busy = true) }
        viewModelScope.launch {
            try { action(); success?.let { channel.send(it) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { channel.send(StickerActionEvent.Failed(failureCode(error))) }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }
}
