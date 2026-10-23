package com.grtsinry43.memedock.feature

import android.graphics.Bitmap
import android.util.Base64
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.core.content.FileProvider
import androidx.test.platform.app.InstrumentationRegistry
import com.grtsinry43.memedock.MainActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import com.grtsinry43.memedock.bridge.generated.ArchiveRestoreMode
import com.grtsinry43.memedock.data.library.trashEntry
import com.grtsinry43.memedock.feature.backup.BackupPhase
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import java.io.File

/** External phase runner owns the disposable emulator, real server and process restarts. */
class CrossPlatformSyncDeviceTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun sharedServerPhase() = runBlocking {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val configFile = File(context.filesDir, "cross-platform.json")
        assumeTrue("Run with tools/testing/android-linux-sync.py", configFile.isFile)
        val config = JSONObject(configFile.readText())
        val phase = config.getString("phase")
        val container = (compose.activity.application as MemeDockApplication).container
        val repository = container.library
        val id = config.getString("sticker")
        suspend fun main(action: () -> Unit) = withContext(Dispatchers.Main) { action() }
        suspend fun idle() = withTimeout(60_000) {
            container.sync.state.first { it.loaded && it.preferencesLoaded && !it.busy }
        }
        suspend fun drain() {
            idle()
            repeat(32) {
                val previous = container.sync.state.value.status.lastSuccessMs ?: 0
                main { container.sync.synchronizeNow() }
                val state = withTimeout(60_000) { container.sync.state.first {
                    !it.busy && ((it.status.lastSuccessMs ?: 0) > previous || it.error != null)
                } }
                assertNull("sync error", state.error)
                assertEquals(0L, state.status.blocked)
                if (state.status.pending == 0L) return
            }
            error("Android queue did not drain")
        }
        suspend fun pause(value: Boolean) {
            idle(); main { container.sync.setPaused(value) }
            withTimeout(60_000) { container.sync.state.first { !it.busy && it.status.paused == value } }
        }
        suspend fun import(name: String, bytes: ByteArray): String {
            val input = repository.createInput()
            try {
                withContext(Dispatchers.IO) { File(input.path).writeBytes(bytes) }
                return repository.importInput(input, name, cancelled = { false }).id
            } finally { input.close() }
        }
        if (phase == "interrupt") {
            withTimeout(60_000) { container.sync.state.first { it.loaded && it.running } }
            repository.patchSticker(repository.detail(id), note = "interrupted-android")
            File(context.filesDir, "cross-interrupt-ready").writeText("running")
            // The host observes this marker, kills this process, resumes the server
            // and launches network-resume to check persisted outbox recovery.
            withTimeout(120_000) { awaitCancellation() }
        }
        idle()
        if (phase == "edit" || phase == "reauthorize") {
            main { container.sync.setWifiOnly(false) }
            idle()
            main { container.sync.connect(config.getString("origin"), config.getString("androidToken")) }
            withTimeout(60_000) { container.sync.state.first { !it.busy && it.hasCredential && it.error == null } }
        } else {
            assertTrue("credential must survive process restart", container.sync.state.value.hasCredential)
        }
        if (phase !in setOf("offline", "resume", "revoked", "transport")) drain()
        when (phase) {
            "edit" -> {
                val detail = repository.detail(id)
                assertEquals("cross-linux", detail.title)
                assertArrayEquals(Base64.decode(config.getString("png"), Base64.DEFAULT), File(detail.previewPath!!).readBytes())
                assertEquals("cross-collection", detail.collection!!.name)
                assertEquals(listOf("cross-tag"), detail.tags.map { it.name })
                repository.patchSticker(detail, title = "android-edited", note = "android-note", starred = true)
                // Exercise relationship writes from Android as well as their Linux-origin reads.
                repository.relations(repository.detail(id), null, emptyList())
                repository.relations(repository.detail(id), detail.collection, detail.tags)
                val bitmap = Bitmap.createBitmap(5, 4, Bitmap.Config.ARGB_8888)
                val bytes = try {
                    bitmap.eraseColor(0xff2a8b4c.toInt())
                    java.io.ByteArrayOutputStream().use { out ->
                        check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)); out.toByteArray()
                    }
                } finally { bitmap.recycle() }
                config.put("androidSticker", import("cross-android.png", bytes))
                drain()
            }
            "offline" -> {
                pause(true)
                repository.patchSticker(repository.detail(id), note = "android-offline")
                assertTrue(container.sync.state.value.status.paused)
            }
            "resume" -> { pause(false); drain() }
            "merged" -> {
                val detail = repository.detail(id)
                assertEquals("linux-offline", detail.title)
                assertEquals("android-offline", detail.note)
            }
            "restore" -> {
                val detail = repository.detail(id)
                assertTrue(detail.deleted)
                assertEquals(1, repository.collections(true).size)
                assertEquals(1, repository.tags(true).size)
                repository.restoreCollection(repository.collections(true).single())
                repository.restoreTag(repository.tags(true).single())
                repository.restoreSticker(detail)
                drain()
            }
            "reimport" -> {
                val page = repository.page("", deleted = true)
                try { assertFalse(page.items.any { it.id == id }) } finally { page.next?.close() }
                assertTrue(repository.collections(true).isEmpty())
                assertTrue(repository.tags(true).isEmpty())
                assertEquals(id, import("cross-fresh.png", Base64.decode(config.getString("png"), Base64.DEFAULT)))
                assertEquals(config.getLong("generation") + 1, repository.detail(id).generation)
                drain()
            }
            "revoked" -> {
                main { container.sync.synchronizeNow() }
                withTimeout(60_000) { container.sync.state.first { !it.busy &&
                    (it.error == "UNAUTHORIZED" || it.status.error == "unauthorized") } }
            }
            "reauthorize" -> { drain(); assertNull(container.sync.state.value.error) }
            "transport" -> {
                repository.patchSticker(repository.detail(id), note = "network-recovered")
                main { container.sync.synchronizeNow() }
                val failed = withTimeout(60_000) { container.sync.state.first { !it.busy && it.error != null } }
                assertTrue(failed.hasCredential)
                assertFalse(failed.status.paused)
                assertNotEquals("UNAUTHORIZED", failed.error)
            }
            "network-resume" -> { assertEquals(config.getString("expectedNote"), repository.detail(id).note) }
            "backup" -> {
                val backup = File(context.filesDir, "share/cross-platform-backup.memedock")
                check(backup.parentFile!!.mkdirs() || backup.parentFile!!.isDirectory)
                val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", backup)
                main { container.backups.create() }
                withTimeout(60_000) { container.backups.state.first { it.phase == BackupPhase.ChooseDestination || it.error != null } }
                assertNull(container.backups.state.value.error)
                main { container.backups.savedTo(uri) }
                withTimeout(60_000) { container.backups.state.first { it.phase == BackupPhase.Saved || it.error != null } }
                assertNull(container.backups.state.value.error)
                assertTrue(backup.length() > 0)
                suspend fun restore(mode: ArchiveRestoreMode, source: android.net.Uri = uri) {
                    main { container.backups.acknowledge(); container.backups.chooseSource(); container.backups.opened(source) }
                    withTimeout(60_000) { container.backups.state.first { it.phase == BackupPhase.Preview || it.error != null } }
                    assertNull(container.backups.state.value.error)
                    main { container.backups.restore(mode) }
                    withTimeout(60_000) { container.backups.state.first { it.phase == BackupPhase.Restored || it.error != null } }
                    assertNull(container.backups.state.value.error)
                    idle()
                }
                val linuxArchive = File(backup.parentFile, "cross-linux-backup.memedock")
                linuxArchive.writeBytes(Base64.decode(config.getString("linuxArchive"), Base64.DEFAULT))
                restore(ArchiveRestoreMode.MERGE, FileProvider.getUriForFile(context, "${context.packageName}.files", linuxArchive))
                assertTrue(container.sync.state.value.hasCredential)
                drain()
                // A pending purge confirmation must be invalidated by replacing the library.
                repository.deleteSticker(repository.detail(id))
                val deleted = repository.detail(id).trashEntry()
                main { container.trash.request(listOf(deleted)) }
                withTimeout(30_000) { container.trash.state.first { it.request != null } }
                val oldBinding = container.sync.state.value.binding
                restore(ArchiveRestoreMode.REPLACE)
                assertNotEquals(oldBinding!!.libraryId, container.sync.state.value.binding!!.libraryId)
                assertFalse(container.sync.state.value.hasCredential)
                assertFalse(container.sync.state.value.status.connected)
                assertNull(container.trash.state.value.request)
                assertFalse(repository.detail(id).deleted)
            }
            else -> error("unknown Android phase: $phase")
        }
        File(context.filesDir, "cross-result.json").writeText(config.toString())
    }
}
