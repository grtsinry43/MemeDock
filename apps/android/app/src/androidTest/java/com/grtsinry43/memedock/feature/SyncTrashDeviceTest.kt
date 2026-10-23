package com.grtsinry43.memedock.feature

import android.graphics.Bitmap
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.lifecycle.Lifecycle
import androidx.test.espresso.Espresso
import androidx.test.platform.app.InstrumentationRegistry
import androidx.work.WorkManager
import com.grtsinry43.memedock.MainActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.platform.credentials.AndroidSyncTokenStore
import com.grtsinry43.memedock.platform.sync.AndroidSyncWorkScheduler
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Assume.assumeNotNull
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.util.concurrent.TimeUnit

class SyncTrashDeviceTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun keystoreCredentialsAreIsolatedByLibraryAndServerAndCanBeCleared() = runBlocking {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val store = AndroidSyncTokenStore(context)
        val library = "credential-test-${System.nanoTime()}"
        val origin = "https://sync.example.test"
        try {
            store.save(library, origin, "device-test-secret")
            assertEquals("device-test-secret", store.load(library, origin))
            assertNull(store.load("other-$library", origin))
            assertNull(store.load(library, "https://other.example.test"))
            val files = context.noBackupFilesDir.listFiles().orEmpty().filter { it.name.startsWith("sync-") }
            assertTrue(files.isNotEmpty())
            assertTrue(files.none { it.readBytes().toString(Charsets.UTF_8).contains("device-test-secret") })
            store.clear(library, origin)
            assertNull(store.load(library, origin))
        } finally { store.clear(library, origin) }
    }

    /** Provision a disposable real-server token via the syncToken instrumentation argument. */
    @Test fun realForegroundAndBackgroundSyncAndConfirmedTrashSnapshot() = runBlocking {
        val args = InstrumentationRegistry.getArguments()
        val token = args.getString("syncToken")
        assumeNotNull(token)
        val origin = args.getString("syncOrigin") ?: "http://127.0.0.1:18787"
        val pixels = System.nanoTime().toInt()
        val container = (compose.activity.application as MemeDockApplication).container
        suspend fun idle() = withTimeout(30_000) { container.sync.state.first { it.loaded && !it.busy } }
        suspend fun synchronizedAfter(time: Long) = withTimeout(45_000) {
            container.sync.state.first { !it.busy && it.status.pending == 0L && (it.status.lastSuccessMs ?: 0) > time }
        }
        suspend fun import(name: String, color: Int): String {
            val input = container.library.createInput()
            try {
                withContext(Dispatchers.IO) {
                    val bitmap = Bitmap.createBitmap(7, 5, Bitmap.Config.ARGB_8888)
                    try {
                        bitmap.eraseColor(color)
                        File(input.path).outputStream().use { check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
                    } finally { bitmap.recycle() }
                }
                return container.library.importInput(input, name, cancelled = { false }).id
            } finally { input.close() }
        }
        idle()
        withContext(Dispatchers.Main) { container.sync.disconnect() }
        withTimeout(30_000) { container.sync.state.first { !it.hasCredential } }
        withContext(Dispatchers.Main) { container.sync.setWifiOnly(false) }
        compose.onNodeWithTag("tab:Mine").performClick()
        compose.onNodeWithTag("settings-sync").performScrollTo().performClick()
        compose.onNodeWithTag("sync-server").performTextReplacement(origin)
        compose.onNodeWithTag("sync-token").performTextReplacement(token!!)
        compose.onNodeWithTag("sync-connect").performScrollTo().performClick()
        synchronizedAfter(0)
        val firstTime = container.sync.state.value.status.lastSuccessMs!!
        val first = import("device-foreground.png", 0xff000000.toInt() or (pixels and 0xffffff))
        synchronizedAfter(firstTime)
        compose.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        val secondTime = container.sync.state.value.status.lastSuccessMs!!
        val second = import("device-background.png", 0xff000000.toInt() or ((pixels xor 0x9e3779) and 0xffffff))
        synchronizedAfter(secondTime)
        val work = WorkManager.getInstance(compose.activity.application)
        val requests = work.getWorkInfosForUniqueWork(AndroidSyncWorkScheduler.ONCE).get(10, TimeUnit.SECONDS)
        assertTrue(requests.isNotEmpty())
        assertNull(container.sync.state.value.error)
        compose.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        container.library.deleteSticker(container.library.detail(first))
        val collection = container.library.createCollection("device-trash-collection")
        val tag = container.library.createTag("device-trash-tag")
        container.library.deleteCollection(collection)
        container.library.deleteTag(tag)
        Espresso.pressBack()
        compose.onNodeWithTag("settings-trash").performScrollTo().performClick()
        compose.onNodeWithTag("trash-more").performClick()
        compose.onNodeWithTag("trash-empty").performClick()
        compose.waitUntil(10_000) { container.trash.state.value.request != null }
        val confirmed = container.trash.state.value.request!!.targets
        assertTrue(confirmed.any { it.id == first })
        assertTrue(confirmed.any { it.id == collection.id })
        assertTrue(confirmed.any { it.id == tag.id })
        // This item enters the trash after the user sees the confirmation.
        container.library.deleteSticker(container.library.detail(second))
        compose.onNodeWithTag("purge-confirm").performClick()
        withTimeout(30_000) { container.trash.state.first { !it.busy && it.request == null } }
        val remaining = container.library.page("", deleted = true)
        try {
            assertTrue(remaining.items.any { it.id == second })
            assertFalse(remaining.items.any { it.id == first })
        } finally { remaining.next?.close() }
        assertFalse(container.library.collections(true).any { it.id == collection.id })
        assertFalse(container.library.tags(true).any { it.id == tag.id })
        withContext(Dispatchers.Main) { container.trash.retryCleanup() }
        withTimeout(30_000) { container.trash.state.first { !it.busy } }
        withContext(Dispatchers.Main) { container.sync.disconnect() }
        withTimeout(30_000) { container.sync.state.first { !it.hasCredential && it.status.paused } }
        Unit
    }
}
