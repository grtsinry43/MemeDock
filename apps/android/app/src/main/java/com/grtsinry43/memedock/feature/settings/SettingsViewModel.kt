package com.grtsinry43.memedock.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.data.settings.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.io.IOException

data class SettingsState(val mode: ThemeMode? = null, val statistics: LibraryStatistics? = null,
    val loading: Boolean = true, val saving: Boolean = false, val error: String? = null, val appearanceError: Boolean = false)

class SettingsViewModel(private val preferences: AppearancePreferences, private val repository: CollectionRepository) : ViewModel() {
    private val mutable = MutableStateFlow(SettingsState())
    val state = mutable.asStateFlow()
    private var statisticsJob: Job? = null
    init {
        viewModelScope.launch {
            preferences.mode.retryWhen { error, _ ->
                if (error is IOException) {
                    mutable.update { it.copy(appearanceError = true) }
                    delay(1_000)
                    true
                } else false
            }.catch { error ->
                if (error is CancellationException) throw error
                mutable.update { it.copy(appearanceError = true) }
            }.collect { mode -> mutable.update { it.copy(mode = mode, appearanceError = false) } }
        }
    }
    fun select(mode: ThemeMode) {
        if (mutable.value.saving) return
        mutable.update { it.copy(saving = true, appearanceError = false) }
        viewModelScope.launch {
            try { preferences.select(mode) }
            catch (cancel: CancellationException) { throw cancel }
            catch (_: Exception) { mutable.update { it.copy(appearanceError = true) } }
            finally { mutable.update { it.copy(saving = false) } }
        }
    }
    fun refresh() {
        statisticsJob?.cancel()
        statisticsJob = viewModelScope.launch {
            mutable.update { it.copy(loading = true, error = null) }
            try { val value = repository.statistics(); mutable.update { it.copy(statistics = value, loading = false) } }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { mutable.update { it.copy(loading = false, error = (error as? LibraryFailure)?.reason ?: "INTERNAL") } }
        }
    }
    fun retry() { repository.retryOpen(); refresh() }
}
