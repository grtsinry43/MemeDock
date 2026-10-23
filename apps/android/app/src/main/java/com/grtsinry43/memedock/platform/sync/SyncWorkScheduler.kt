package com.grtsinry43.memedock.platform.sync

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import androidx.work.*
import com.grtsinry43.memedock.data.sync.SyncWorkIdentity
import com.grtsinry43.memedock.data.sync.SyncWorkScheduler
import java.util.concurrent.TimeUnit

class AndroidSyncWorkScheduler(context: Context) : SyncWorkScheduler {
    private val work = WorkManager.getInstance(context)
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)
    private var configured: Pair<SyncWorkIdentity, Boolean>? = null

    override fun configure(identity: SyncWorkIdentity?, wifiOnly: Boolean) {
        val next = identity?.let { it to wifiOnly }
        if (configured == next && next != null) return
        if (configured != null && configured != next) work.cancelUniqueWork(ONCE)
        configured = next
        if (identity == null) {
            work.cancelUniqueWork(ONCE)
            work.cancelUniqueWork(PERIODIC)
        } else {
            val request = PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
                .setInputData(identity.data()).setConstraints(constraints(wifiOnly))
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build()
            work.enqueueUniquePeriodicWork(PERIODIC, ExistingPeriodicWorkPolicy.UPDATE, request)
        }
    }
    override fun request(identity: SyncWorkIdentity, wifiOnly: Boolean) {
        val request = OneTimeWorkRequestBuilder<SyncWorker>()
            .setInputData(identity.data()).setConstraints(constraints(wifiOnly))
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build()
        work.enqueueUniqueWork(ONCE, ExistingWorkPolicy.KEEP, request)
    }
    override fun automaticNetworkAvailable(wifiOnly: Boolean): Boolean {
        val capabilities = connectivity.getNetworkCapabilities(connectivity.activeNetwork) ?: return false
        return capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
            capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED) &&
            (!wifiOnly || capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED))
    }
    companion object {
        const val ONCE = "memedock-sync-pending"
        const val PERIODIC = "memedock-sync-periodic"
        internal fun constraints(wifiOnly: Boolean) = Constraints.Builder()
            .setRequiredNetworkType(if (wifiOnly) NetworkType.UNMETERED else NetworkType.CONNECTED).build()
        internal fun SyncWorkIdentity.data() = workDataOf("library" to libraryId, "origin" to origin, "device" to deviceId)
        internal fun identity(data: Data): SyncWorkIdentity? {
            val library = data.getString("library") ?: return null
            val origin = data.getString("origin") ?: return null
            val device = data.getString("device") ?: return null
            return SyncWorkIdentity(library, origin, device)
        }
    }
}
