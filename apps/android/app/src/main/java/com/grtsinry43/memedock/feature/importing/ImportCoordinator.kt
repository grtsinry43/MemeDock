package com.grtsinry43.memedock.feature.importing

import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.platform.importing.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.util.concurrent.atomic.AtomicBoolean

/** Retained by the application session; page recreation never repeats a batch. */
class ImportCoordinator(private val repository: LibraryRepository, private val gateway: ImportGateway, private val scope: CoroutineScope) {
    private val mutable = MutableStateFlow(ImportState())
    val state = mutable.asStateFlow()
    private val cancelled = AtomicBoolean(false)

    fun prepare(uris: List<String>) {
        if (mutable.value.busy || uris.isEmpty()) return
        if (uris.size > 200) {
            mutable.value = ImportState(visible = true, selectionError = "BATCH_LIMIT")
            return
        }
        mutable.value = ImportState(phase = ImportPhase.Preparing, visible = true)
        scope.launch {
            val items = uris.distinct().map { uri ->
                try { ImportItemState(gateway.describe(uri)) }
                catch (cancel: CancellationException) { throw cancel }
                catch (error: Exception) { ImportItemState(ImportCandidate(uri, "未命名图片", null), ImportItemStatus.Failed, error = reason(error)) }
            }
            mutable.update { it.copy(phase = ImportPhase.Review, items = items) }
        }
    }
    fun show() { mutable.update { it.copy(visible = true) } }
    fun hide() { mutable.update { it.copy(visible = false) } }
    fun cancel() {
        cancelled.set(true)
        mutable.update { it.copy(cancellationRequested = true) }
        scope.launch {
            try { gateway.cancelActiveRead() }
            catch (cancel: CancellationException) { throw cancel }
            catch (_: java.io.IOException) { /* The read may already have closed during cancellation. */ }
        }
    }
    fun retryFailed() {
        if (mutable.value.busy) return
        mutable.update { state -> state.copy(items = state.items.map { item ->
            if (item.status == ImportItemStatus.Failed || item.status == ImportItemStatus.Cancelled) item.copy(status = ImportItemStatus.Queued, error = null, bytesRead = 0) else item
        }) }
        start()
    }
    fun start() {
        if (mutable.value.busy || mutable.value.items.none { it.status == ImportItemStatus.Queued }) return
        cancelled.set(false)
        mutable.update { it.copy(phase = ImportPhase.Running, cancellationRequested = false) }
        scope.launch {
            for (index in mutable.value.items.indices) {
                val item = mutable.value.items[index]
                if (item.status != ImportItemStatus.Queued) continue
                if (cancelled.get()) { update(index, ImportItemStatus.Cancelled); continue }
                var input: ImportSlot? = null
                try {
                    input = repository.createInput()
                    update(index, ImportItemStatus.Reading)
                    gateway.copy(item.candidate, input.path, cancelled::get) { bytes ->
                        mutable.update { state -> state.copy(items = state.items.mapIndexed { i, value -> if (i == index) value.copy(bytesRead = bytes) else value }) }
                    }
                    if (cancelled.get()) throw LibraryFailure("CANCELLED")
                    update(index, ImportItemStatus.Validating)
                    val result = repository.importInput(input, item.candidate.name, cancelled::get)
                    update(index, when (result.disposition) {
                        ImportDisposition.Created -> ImportItemStatus.Created
                        ImportDisposition.Reused -> ImportItemStatus.Reused
                        ImportDisposition.RestoreRequired -> ImportItemStatus.RestoreRequired
                    })
                } catch (cancel: CancellationException) {
                    update(index, ImportItemStatus.Cancelled)
                    throw cancel
                } catch (error: Exception) {
                    val code = reason(error)
                    update(index, if (cancelled.get() || code == "CANCELLED") ImportItemStatus.Cancelled else ImportItemStatus.Failed, code)
                } finally {
                    input?.let { slot ->
                        try { withContext(NonCancellable) { repository.discardInput(slot) } }
                        catch (error: Exception) { update(index, ImportItemStatus.Failed, reason(error)) }
                        finally { slot.close() }
                    }
                }
            }
            mutable.update { it.copy(phase = ImportPhase.Finished) }
        }
    }
    private fun update(index: Int, status: ImportItemStatus, error: String? = null) {
        mutable.update { state -> state.copy(items = state.items.mapIndexed { i, item -> if (i == index) item.copy(status = status, error = error) else item }) }
    }
    private fun reason(error: Exception): String = when (error) {
        is LibraryFailure -> error.reason
        is SecurityException -> "PERMISSION_DENIED"
        is java.io.IOException -> "IO"
        else -> "INTERNAL"
    }
}
