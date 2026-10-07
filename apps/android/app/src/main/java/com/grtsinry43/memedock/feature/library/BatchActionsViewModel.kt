package com.grtsinry43.memedock.feature.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.failureCode
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

data class BatchSelectionState(
    val selecting: Boolean = false,
    val selected: Map<String, LibraryItem> = emptyMap(),
    val busy: Boolean = false,
    val report: BatchReport? = null,
    val error: String? = null,
)

class BatchActionsViewModel(private val repository: ManagementRepository) : ViewModel() {
    private val mutable = MutableStateFlow(BatchSelectionState())
    val state = mutable.asStateFlow()
    private var operation: BatchOperation? = null
    private var stopRequested = false
    private var exitAfterBatch = false
    fun start(item: LibraryItem? = null) {
        if (mutable.value.busy) return
        mutable.update { it.copy(selecting = true, report = null, error = null,
            selected = if (item == null) it.selected else it.selected + (item.id to item)) }
    }
    fun exit() { if (!mutable.value.busy) mutable.value = BatchSelectionState() }
    fun leave() {
        if (mutable.value.busy) { exitAfterBatch = true; stop() } else exit()
    }
    fun toggle(item: LibraryItem) {
        if (mutable.value.busy) return
        mutable.update { it.copy(selected = if (item.id in it.selected) it.selected - item.id else it.selected + (item.id to item), report = null, error = null) }
    }
    fun run(action: BatchAction) {
        val targets = mutable.value.selected.values.toList()
        if (mutable.value.busy || targets.isEmpty()) return
        stopRequested = false
        exitAfterBatch = false
        mutable.update { it.copy(busy = true, report = null, error = null) }
        viewModelScope.launch {
            try {
                repository.batch(targets, action).use { batch ->
                    operation = batch
                    if (stopRequested) batch.cancel()
                    val polling = launch { while (isActive) { mutable.update { it.copy(report = batch.snapshot()) }; delay(150) } }
                    val report = try { batch.awaitResult() } finally { polling.cancelAndJoin() }
                    val retry = report.items.filter { it.outcome == BatchOutcome.Failed || it.outcome == BatchOutcome.Pending }.map { it.id }.toSet()
                    mutable.update { it.copy(selected = it.selected.filterKeys { id -> id in retry }, report = report) }
                }
            } catch (cancel: CancellationException) { operation?.cancel(); throw cancel }
            catch (error: Exception) { mutable.update { it.copy(error = failureCode(error)) } }
            finally {
                operation = null
                if (exitAfterBatch) mutable.value = BatchSelectionState()
                else mutable.update { it.copy(busy = false) }
            }
        }
    }
    fun stop() { stopRequested = true; operation?.cancel() }
    override fun onCleared() { stop(); super.onCleared() }
}
