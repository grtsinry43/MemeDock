package com.grtsinry43.memedock.feature.backup

import android.net.Uri
import com.grtsinry43.memedock.bridge.generated.*
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.platform.backup.AndroidBackupGateway
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.util.concurrent.atomic.AtomicBoolean

enum class BackupPhase { Idle, Creating, ChooseDestination, Saving, ChooseSource, Reading, Preview, Restoring, Saved, Restored }
data class BackupState(val phase: BackupPhase = BackupPhase.Idle, val summary: ArchiveSummary? = null, val error: String? = null) {
    val busy get() = phase in listOf(BackupPhase.Creating, BackupPhase.ChooseDestination, BackupPhase.Saving,
        BackupPhase.ChooseSource, BackupPhase.Reading, BackupPhase.Restoring)
}
sealed interface BackupPicker {
    data class Save(val fileName: String) : BackupPicker
    data object Open : BackupPicker
}

/** Process-owned: activity recreation does not abandon a pending picker or file. */
class BackupCoordinator(private val repository: BackupRepository, private val gateway: AndroidBackupGateway,
    private val scope: CoroutineScope) {
    private val mutable = MutableStateFlow(BackupState())
    val state = mutable.asStateFlow()
    private var output: BackupFileHandle? = null
    private var preview: PreparedArchiveHandle? = null
    private var pickerClaimed = false
    private var job: Job? = null
    private val cancelled = AtomicBoolean(false)
    @Volatile private var restoreTask: ArchiveRestoreTask? = null
    @Volatile private var inspectionTask: ArchiveInspectionTask? = null

    fun create() {
        if (mutable.value.busy || preview != null) return
        cancelled.set(false)
        mutable.value = BackupState(BackupPhase.Creating)
        job = scope.launch {
            try {
                output = repository.create()
                pickerClaimed = false
                mutable.value = BackupState(BackupPhase.ChooseDestination)
            } catch (_: CancellationException) { mutable.value = BackupState() }
            catch (error: Exception) { failed(error) }
        }
    }
    fun chooseSource() {
        if (mutable.value.busy || preview != null) return
        cancelled.set(false)
        pickerClaimed = false
        mutable.value = BackupState(BackupPhase.ChooseSource)
    }
    fun claimPicker(): BackupPicker? {
        if (pickerClaimed) return null
        val request = when (mutable.value.phase) {
            BackupPhase.ChooseDestination -> output?.let { BackupPicker.Save(it.fileName()) }
            BackupPhase.ChooseSource -> BackupPicker.Open
            else -> null
        }
        if (request != null) pickerClaimed = true
        return request
    }
    fun pickerFailed(error: Exception) {
        if (mutable.value.phase == BackupPhase.ChooseDestination) savedTo(null)
        failed(error)
    }
    fun savedTo(uri: Uri?) {
        val value = output
        if (value == null) {
            if (uri != null) scope.launch { try { gateway.discard(uri) } catch (error: Exception) { failed(error) } }
            return
        }
        if (mutable.value.phase != BackupPhase.ChooseDestination) return
        output = null
        mutable.value = BackupState(if (uri == null) BackupPhase.Idle else BackupPhase.Saving)
        job = scope.launch {
            try {
                if (uri != null) {
                    gateway.save(uri, value.path(), value.byteSize().toLong(), cancelled::get)
                    mutable.value = BackupState(BackupPhase.Saved)
                }
            } catch (_: CancellationException) { mutable.value = BackupState() }
            catch (error: Exception) { failed(error) }
            finally { try { repository.discard(value) } catch (error: Exception) { failed(error) } }
        }
    }
    fun opened(uri: Uri?) {
        if (mutable.value.phase != BackupPhase.ChooseSource) return
        if (uri == null) { mutable.value = BackupState(); return }
        mutable.value = BackupState(BackupPhase.Reading)
        job = scope.launch {
            var input: ArchiveInputHandle? = null
            try {
                val staging = repository.input().also { input = it }
                gateway.read(uri, staging.path(), defaultResourceConfiguration().maxArchiveBytes.toLong(), cancelled::get)
                // Inspection owns the input after submission and cleans invalid input.
                input = null
                val prepared = try { repository.inspect(staging) { task ->
                    inspectionTask = task
                    if (cancelled.get()) task?.cancel()
                } } finally { staging.close() }
                preview = prepared
                mutable.value = BackupState(BackupPhase.Preview, prepared.summary())
            } catch (_: CancellationException) { mutable.value = BackupState() }
            catch (error: Exception) { failed(error) }
            finally { input?.let { try { repository.discard(it) } catch (error: Exception) { failed(error) } } }
        }
    }
    fun restore(mode: ArchiveRestoreMode) {
        if (mutable.value.phase != BackupPhase.Preview) return
        val value = preview ?: return
        preview = null
        mutable.value = mutable.value.copy(phase = BackupPhase.Restoring, error = null)
        job = scope.launch {
            try {
                val summary = repository.restore(value, mode) { restoreTask = it }
                mutable.value = BackupState(BackupPhase.Restored, summary)
            } catch (_: CancellationException) { mutable.value = BackupState() }
            catch (error: Exception) { failed(error) }
            finally {
                // Native restore consumes the preview and releases its input.
                value.close()
            }
        }
    }
    fun dismissPreview() {
        val value = preview ?: return
        preview = null
        mutable.value = BackupState(BackupPhase.Reading)
        job = scope.launch {
            try { repository.discard(value); mutable.value = BackupState() }
            catch (error: Exception) { failed(error) }
        }
    }
    fun cancel() {
        cancelled.set(true)
        if (mutable.value.phase == BackupPhase.Restoring) restoreTask?.cancel()
        else if (mutable.value.phase == BackupPhase.Reading) inspectionTask?.cancel()
        else if (mutable.value.phase == BackupPhase.Creating) job?.cancel()
    }
    private fun failed(error: Exception) {
        if (error is LibraryFailure && error.reason == "CANCELLED") { mutable.value = BackupState(); return }
        mutable.value = BackupState(error = when (error) {
            is LibraryFailure -> error.reason
            is SecurityException -> "PERMISSION_DENIED"
            is java.io.IOException -> "IO"
            is android.content.ActivityNotFoundException -> "NO_SAVE_TARGET"
            else -> "INTERNAL"
        })
    }
}
