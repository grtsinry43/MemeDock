package com.grtsinry43.memedock.feature.trash

import com.grtsinry43.memedock.app.LibraryBinding
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.failureCode
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

data class PurgeRequest(val binding: LibraryBinding, val targets: List<TrashEntry>)
data class PurgeReport(val succeeded: Set<TrashEntry>, val failed: Int, val error: String?)
data class PurgeState(val request: PurgeRequest? = null, val busy: Boolean = false,
    val completed: Int = 0, val total: Int = 0, val report: PurgeReport? = null,
    val error: String? = null, val cleanupDone: Boolean = false, val empty: Boolean = false)

class TrashCoordinator(private val repository: TrashRepository, private val scope: CoroutineScope) {
    private val mutable = MutableStateFlow(PurgeState())
    val state = mutable.asStateFlow()
    private var operation: Job? = null
    private var maintenance = false
    private var completion: ((Set<TrashEntry>) -> Unit)? = null
    init {
        scope.launch {
            repository.bindings.collect { binding ->
                val request = mutable.value.request
                if (request != null && request.binding != binding) {
                    mutable.update { it.copy(request = null, error = "CONFLICT") }
                    completion = null
                }
            }
        }
    }
    fun request(targets: List<TrashEntry>, completed: (Set<TrashEntry>) -> Unit = {}) = prepare {
        if (targets.isEmpty()) return@prepare null
        completion = completed
        PurgeRequest(repository.binding(), targets.distinct().toList())
    }
    fun empty() = prepare {
        val binding = repository.binding()
        val targets = repository.snapshot(binding)
        if (repository.binding() != binding) throw LibraryFailure("CONFLICT")
        completion = null
        if (targets.isEmpty()) null else PurgeRequest(binding, targets)
    }
    private fun prepare(create: suspend () -> PurgeRequest?) {
        if (maintenance || mutable.value.busy || mutable.value.request != null) return
        mutable.value = PurgeState(busy = true)
        operation = scope.launch {
            try { val request = create(); mutable.update { it.copy(request = request, empty = request == null) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(error = failureCode(error)) } }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }
    fun dismiss() { if (!mutable.value.busy) { mutable.update { it.copy(request = null) }; completion = null } }
    fun acknowledge() { mutable.update { it.copy(report = null, error = null, cleanupDone = false, empty = false) } }
    fun confirm() {
        val request = mutable.value.request ?: return
        if (maintenance || mutable.value.busy) return
        mutable.update { it.copy(busy = true, total = request.targets.size, completed = 0) }
        val callback = completion
        completion = null
        operation = scope.launch {
            val succeeded = mutableSetOf<TrashEntry>()
            var error: String? = null
            try {
                for (entry in request.targets) {
                    if (repository.binding() != request.binding) { error = "CONFLICT"; break }
                    try { repository.purge(request.binding, entry); succeeded += entry }
                    catch (cancel: CancellationException) { throw cancel }
                    catch (failure: Exception) { error = failureCode(failure) }
                    mutable.update { it.copy(completed = it.completed + 1) }
                }
                mutable.update { it.copy(report = PurgeReport(succeeded.toSet(), request.targets.size - succeeded.size, error)) }
                callback?.invoke(succeeded.toSet())
            } catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { mutable.update { it.copy(error = failureCode(failure)) } }
            finally { mutable.update { it.copy(busy = false, request = null) } }
        }
    }
    fun retryCleanup() {
        if (maintenance || mutable.value.busy || mutable.value.request != null) return
        mutable.value = PurgeState(busy = true)
        operation = scope.launch {
            try { repository.cleanup(repository.binding()); mutable.update { it.copy(cleanupDone = true) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(error = failureCode(error)) } }
            finally { mutable.update { it.copy(busy = false) } }
        }
    }
    suspend fun beginMaintenance() = withContext(Dispatchers.Main.immediate) {
        maintenance = true
        operation?.cancelAndJoin()
        completion = null
        mutable.value = PurgeState()
    }
    suspend fun endMaintenance() = withContext(Dispatchers.Main.immediate) { maintenance = false }
}
