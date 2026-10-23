package com.grtsinry43.memedock.platform.sync

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.grtsinry43.memedock.app.MemeDockApplication
import com.grtsinry43.memedock.feature.sync.SyncRun
import com.grtsinry43.memedock.ui.failureCode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.withTimeoutOrNull

class SyncWorker(context: Context, parameters: WorkerParameters) : CoroutineWorker(context, parameters) {
    override suspend fun doWork(): Result {
        val identity = AndroidSyncWorkScheduler.identity(inputData) ?: return Result.failure()
        val coordinator = (applicationContext as MemeDockApplication).container.sync
        return try {
            // Leave time for native cancellation/cleanup before WorkManager's ten-minute limit.
            when (withTimeoutOrNull(8 * 60_000L) { coordinator.background(identity) }) {
                SyncRun.Complete, SyncRun.Skipped -> Result.success()
                SyncRun.Retry, null -> Result.retry()
                SyncRun.Blocked -> Result.failure()
            }
        } catch (cancel: CancellationException) { throw cancel }
        catch (error: Exception) {
            if (failureCode(error).uppercase() in setOf("IO", "BUSY", "NETWORK", "TIMEOUT")) Result.retry() else Result.failure()
        }
    }
}
