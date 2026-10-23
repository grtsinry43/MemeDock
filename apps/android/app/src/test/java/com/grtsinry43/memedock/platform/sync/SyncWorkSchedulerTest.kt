package com.grtsinry43.memedock.platform.sync

import androidx.work.Data
import androidx.work.NetworkType
import com.grtsinry43.memedock.data.sync.SyncWorkIdentity
import com.grtsinry43.memedock.platform.sync.AndroidSyncWorkScheduler.Companion.data
import org.junit.Assert.*
import org.junit.Test

class SyncWorkSchedulerTest {
    @Test fun durableRequestsContainIdentityButNoCredentialsAndRejectIncompleteIdentity() {
        val identity = SyncWorkIdentity("library", "https://example.test", "device")
        val input = identity.data()
        assertEquals(setOf("library", "origin", "device"), input.keyValueMap.keys)
        assertEquals(identity, AndroidSyncWorkScheduler.identity(input))
        assertNull(AndroidSyncWorkScheduler.identity(Data.EMPTY))
        assertNull(AndroidSyncWorkScheduler.identity(Data.Builder().putString("library", "library").build()))
    }
    @Test fun automaticNetworkPolicyRequiresConnectivityAndHonorsUnmeteredChoice() {
        assertEquals(NetworkType.CONNECTED, AndroidSyncWorkScheduler.constraints(false).requiredNetworkType)
        assertEquals(NetworkType.UNMETERED, AndroidSyncWorkScheduler.constraints(true).requiredNetworkType)
    }
}
