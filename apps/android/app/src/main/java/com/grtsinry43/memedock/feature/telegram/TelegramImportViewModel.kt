package com.grtsinry43.memedock.feature.telegram

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.telegram.*
import com.grtsinry43.memedock.platform.credentials.TelegramTokenStore
import com.grtsinry43.memedock.ui.failureCode
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import java.util.concurrent.atomic.AtomicBoolean

data class TelegramImportState(
    val token: String = "",
    val name: String = "",
    val rememberToken: Boolean = false,
    val credentialsBusy: Boolean = true,
    val credentialError: Boolean = false,
    val pack: TelegramPack? = null,
    val selected: Set<String> = emptySet(),
    val itemStates: Map<String, TelegramItemState> = emptyMap(),
    val loading: Boolean = false,
    val importing: Boolean = false,
    val stopping: Boolean = false,
    val report: TelegramReport? = null,
    val error: String? = null,
) {
    val busy get() = loading || importing || credentialsBusy
    // Never include credentials in diagnostic output.
    override fun toString() = "TelegramImportState(loading=$loading, importing=$importing, selected=${selected.size})"
}

class TelegramImportViewModel(private val repository: TelegramRepository, private val tokens: TelegramTokenStore) : ViewModel() {
    private val mutable = MutableStateFlow(TelegramImportState())
    val state = mutable.asStateFlow()
    private var job: Job? = null
    private val stop = AtomicBoolean(false)
    private val previews = Semaphore(2)

    init {
        viewModelScope.launch {
            try {
                val token = tokens.load()
                mutable.update { it.copy(token = token.orEmpty(), rememberToken = token != null) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (_: Exception) { mutable.update { it.copy(credentialError = true) } }
            finally { mutable.update { it.copy(credentialsBusy = false) } }
        }
    }
    fun token(value: String) { if (!state.value.busy) mutable.update { it.copy(token = value.take(256), error = null) } }
    fun name(value: String) { if (!state.value.busy) mutable.update { it.copy(name = value.take(1024), error = null) } }
    fun remember(value: Boolean) {
        if (state.value.busy) return
        mutable.update { it.copy(rememberToken = value) }
        if (!value) clearToken(clearInput = false)
    }
    fun forget() = clearToken(clearInput = true)
    private fun clearToken(clearInput: Boolean) {
        if (state.value.busy) return
        mutable.update { it.copy(credentialsBusy = true, rememberToken = false) }
        viewModelScope.launch {
            try { tokens.clear(); mutable.update { it.copy(credentialError = false, token = if (clearInput) "" else it.token) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (_: Exception) { mutable.update { it.copy(credentialError = true) } }
            finally { mutable.update { it.copy(credentialsBusy = false) } }
        }
    }
    fun load() {
        val input = state.value
        if (input.busy || input.token.isBlank() || input.name.isBlank()) return
        mutable.update { it.copy(loading = true, error = null, report = null) }
        job = viewModelScope.launch {
            try {
                val pack = repository.load(input.token.trim(), input.name.trim())
                mutable.value.pack?.close()
                mutable.update { it.copy(pack = pack, selected = emptySet(), itemStates = emptyMap()) }
                if (input.rememberToken) {
                    try { tokens.save(input.token.trim()); mutable.update { it.copy(credentialError = false) } }
                    catch (cancel: CancellationException) { throw cancel }
                    catch (_: Exception) { mutable.update { it.copy(credentialError = true) } }
                }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(error = failureCode(error)) } }
            finally { mutable.update { it.copy(loading = false) } }
        }
    }
    fun toggle(id: String) {
        val current = state.value
        if (current.busy || current.pack?.items?.none { it.id == id &&
            (current.itemStates[id] ?: it.state) != TelegramItemState.RestoreRequired } != false) return
        mutable.update { it.copy(selected = if (id in it.selected) it.selected - id else it.selected + id, report = null) }
    }
    fun selectAll() {
        if (state.value.busy) return
        mutable.update { it.copy(selected = it.pack?.items?.filter { item ->
            (it.itemStates[item.id] ?: item.state) in setOf(TelegramItemState.Available, TelegramItemState.OriginalMissing)
        }?.map { item -> item.id }?.toSet().orEmpty(), report = null) }
    }
    fun clearSelection() { if (!state.value.busy) mutable.update { it.copy(selected = emptySet(), report = null) } }
    fun changePack() {
        if (state.value.busy) return
        mutable.value.pack?.close()
        mutable.update { it.copy(pack = null, selected = emptySet(), itemStates = emptyMap(), report = null, error = null) }
    }
    suspend fun preview(pack: TelegramPack, id: String) = previews.withPermit { repository.preview(pack, id) }
    fun importSelected() {
        val current = state.value
        val pack = current.pack ?: return
        if (current.busy || current.selected.isEmpty()) return
        stop.set(false)
        val ids = current.selected.toList()
        mutable.update { it.copy(importing = true, stopping = false, error = null,
            report = TelegramReport(ids.map { id -> TelegramResult(id, TelegramOutcome.Pending) }, false)) }
        job = viewModelScope.launch {
            try {
                val report = repository.import(pack, ids, stop::get) { progress -> mutable.update { it.copy(report = progress) } }
                mutable.update { it.copy(report = report, itemStates = it.itemStates + report.items.mapNotNull { item ->
                    when (item.outcome) {
                        TelegramOutcome.Created, TelegramOutcome.Reused -> item.id to TelegramItemState.Imported
                        TelegramOutcome.RestoreRequired -> item.id to TelegramItemState.RestoreRequired
                        else -> null
                    }
                }.toMap(), selected = report.items.filter { item ->
                    item.outcome == TelegramOutcome.Failed || item.outcome == TelegramOutcome.Pending
                }.map { item -> item.id }.toSet()) }
            } catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(error = failureCode(error)) } }
            finally { mutable.update { it.copy(importing = false, stopping = false) } }
        }
    }
    fun cancel() {
        if (state.value.importing) { stop.set(true); mutable.update { it.copy(stopping = true) } }
        else if (state.value.loading) job?.cancel()
    }
    override fun onCleared() {
        stop.set(true)
        job?.cancel()
        mutable.value.pack?.close()
    }
}
