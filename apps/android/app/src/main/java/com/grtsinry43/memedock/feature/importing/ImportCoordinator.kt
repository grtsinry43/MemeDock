package com.grtsinry43.memedock.feature.importing

import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.platform.importing.*
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Retained by the application session; page recreation never repeats a batch. Batches picked in the app start
 * right away; [eager] batches arrive from other apps, are staged first and wait for review.
 */
class ImportCoordinator(private val repository: LibraryRepository, private val gateway: ImportGateway, private val scope: CoroutineScope) {
    private val mutable = MutableStateFlow(ImportState())
    val state = mutable.asStateFlow()
    private val channel = Channel<ImportReport>(Channel.BUFFERED)
    /** One report per finished run, consumed once. */
    val reports = channel.receiveAsFlow()
    private val cancelled = AtomicBoolean(false)
    private val staged = mutableMapOf<Int, ImportSlot>()
    private var eager = false

    fun selectionFailed(reason: String) { mutable.update { it.copy(selectionError = reason) } }
    fun clearSelectionError() { mutable.update { it.copy(selectionError = null) } }
    fun prepare(uris: List<String>, eager: Boolean = false) {
        if (uris.isEmpty()) return
        if (mutable.value.busy || mutable.value.items.any { it.status == ImportItemStatus.Staged || it.status == ImportItemStatus.Queued }) {
            selectionFailed("BATCH_BUSY"); return
        }
        if (uris.size > 200) {
            selectionFailed("BATCH_LIMIT")
            return
        }
        mutable.value = ImportState(phase = ImportPhase.Preparing, visible = eager)
        this.eager = eager
        cancelled.set(false)
        scope.launch {
            val items = uris.distinct().map { uri ->
                try {
                    if (cancelled.get()) ImportItemState(ImportCandidate(uri, "未命名图片", null), ImportItemStatus.Cancelled)
                    else ImportItemState(gateway.describe(uri))
                }
                catch (cancel: CancellationException) { throw cancel }
                catch (error: Exception) { ImportItemState(ImportCandidate(uri, "未命名图片", null), if (cancelled.get()) ImportItemStatus.Cancelled else ImportItemStatus.Failed, error = reason(error)) }
            }
            mutable.update { it.copy(items = items) }
            if (eager) {
                for (index in items.indices) {
                    if (items[index].status != ImportItemStatus.Queued) continue
                    if (cancelled.get()) { update(index, ImportItemStatus.Cancelled); continue }
                    try { hold(index, items[index]) }
                    catch (cancel: CancellationException) { throw cancel }
                    catch (error: Exception) { update(index, if (cancelled.get()) ImportItemStatus.Cancelled else ImportItemStatus.Failed, reason(error)) }
                }
            }
            if (cancelled.get()) discardSlots()
            when {
                eager -> mutable.update { it.copy(phase = ImportPhase.Review) }
                // Goes straight to running so the page never sees a review step for in-app picks.
                mutable.value.ready > 0 && !cancelled.get() -> run()
                else -> finish()
            }
        }
    }
    fun show() { mutable.update { it.copy(visible = true) } }
    fun selectCollection(collection: LibraryCollection?) {
        if (!mutable.value.busy) mutable.update { it.copy(collection = collection) }
    }
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
    fun discard() {
        if (mutable.value.busy) { cancel(); return }
        mutable.update { it.copy(phase = ImportPhase.Preparing) }
        scope.launch {
            discardSlots()
            mutable.update { state -> state.copy(visible = false, phase = ImportPhase.Finished,
                items = state.items.map { if (it.status == ImportItemStatus.Staged || it.status == ImportItemStatus.Queued) it.copy(status = ImportItemStatus.Cancelled) else it }) }
        }
    }
    fun retryFailed() {
        if (mutable.value.busy) return
        mutable.update { state -> state.copy(items = state.items.map { item ->
            if (item.status == ImportItemStatus.Failed || item.status == ImportItemStatus.Cancelled) item.copy(status = ImportItemStatus.Queued, error = null, bytesRead = 0) else item
        }) }
        if (eager && mutable.value.phase == ImportPhase.Review) {
            cancelled.set(false)
            mutable.update { it.copy(phase = ImportPhase.Preparing, cancellationRequested = false) }
            scope.launch {
                for (index in mutable.value.items.indices) {
                    val item = mutable.value.items[index]
                    if (item.status != ImportItemStatus.Queued) continue
                    try { hold(index, item) }
                    catch (cancel: CancellationException) { throw cancel }
                    catch (error: Exception) { update(index, if (cancelled.get()) ImportItemStatus.Cancelled else ImportItemStatus.Failed, reason(error)) }
                }
                mutable.update { it.copy(phase = ImportPhase.Review) }
            }
        } else start()
    }
    fun start() {
        if (mutable.value.busy || mutable.value.ready == 0) return
        run()
    }
    private fun run() {
        cancelled.set(false)
        mutable.update { it.copy(phase = ImportPhase.Running, cancellationRequested = false) }
        scope.launch {
            for (index in mutable.value.items.indices) {
                val item = mutable.value.items[index]
                if (item.status != ImportItemStatus.Queued && item.status != ImportItemStatus.Staged) continue
                if (cancelled.get()) {
                    staged.remove(index)?.let { release(it, index) }
                    update(index, ImportItemStatus.Cancelled); continue
                }
                var input: ImportSlot? = null
                try {
                    input = staged.remove(index) ?: stage(index, item)
                    if (cancelled.get()) throw LibraryFailure("CANCELLED")
                    update(index, ImportItemStatus.Validating)
                    val result = repository.importInput(input, item.candidate.name, mutable.value.collection?.id, cancelled::get)
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
                    input?.let { release(it, index) }
                }
            }
            finish()
        }
    }
    private suspend fun finish() {
        val finished = mutable.updateAndGet { it.copy(phase = ImportPhase.Finished) }
        channel.send(ImportReport(finished.count(ImportItemStatus.Created), finished.count(ImportItemStatus.Reused),
            finished.unresolved, stopped = cancelled.get()))
    }
    private suspend fun stage(index: Int, item: ImportItemState): ImportSlot {
        val input = repository.createInput()
        try {
            update(index, ImportItemStatus.Reading)
            gateway.copy(item.candidate, input.path, cancelled::get) { bytes ->
                mutable.update { state -> state.copy(items = state.items.mapIndexed { i, value -> if (i == index) value.copy(bytesRead = bytes) else value }) }
            }
            if (cancelled.get()) throw LibraryFailure("CANCELLED")
            return input
        } catch (error: Exception) { release(input, index); throw error }
    }
    private suspend fun hold(index: Int, item: ImportItemState) {
        val slot = stage(index, item)
        staged[index] = slot
        edit(index) { it.copy(status = ImportItemStatus.Staged, error = null, stagedPath = slot.path) }
    }
    private suspend fun release(slot: ImportSlot, index: Int) {
        edit(index) { it.copy(stagedPath = null) }
        try { withContext(NonCancellable) { repository.discardInput(slot) } }
        catch (error: Exception) { update(index, ImportItemStatus.Failed, reason(error)) }
        finally { slot.close() }
    }
    private fun edit(index: Int, change: (ImportItemState) -> ImportItemState) {
        mutable.update { state -> state.copy(items = state.items.mapIndexed { i, item -> if (i == index) change(item) else item }) }
    }
    private suspend fun discardSlots() {
        val pending = staged.toMap(); staged.clear()
        pending.forEach { (index, slot) -> release(slot, index) }
        mutable.update { state -> state.copy(items = state.items.map { if (it.status == ImportItemStatus.Staged) it.copy(status = ImportItemStatus.Cancelled) else it }) }
    }
    private fun update(index: Int, status: ImportItemStatus, error: String? = null) = edit(index) { it.copy(status = status, error = error) }
    private fun reason(error: Exception): String = when (error) {
        is LibraryFailure -> error.reason
        is SecurityException -> "PERMISSION_DENIED"
        is java.io.IOException -> "IO"
        else -> "INTERNAL"
    }
}
