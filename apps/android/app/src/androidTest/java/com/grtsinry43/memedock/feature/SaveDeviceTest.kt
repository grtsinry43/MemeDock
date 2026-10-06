package com.grtsinry43.memedock.feature

import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import com.grtsinry43.memedock.MainActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.platform.saving.AndroidSaveGateway
import com.grtsinry43.memedock.platform.saving.SaveCoordinator
import com.grtsinry43.memedock.data.library.DetailRepository
import com.grtsinry43.memedock.data.library.OutputLease
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File

class SaveDeviceTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()
    @Test fun safWritesExactOutputAndRemovesNewPartialDocumentOnWriteFailure() = runBlocking {
        withTimeout(40_000) {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val container = (context.applicationContext as MemeDockApplication).container
            val input = container.library.createInput()
            val bitmap = Bitmap.createBitmap(512, 256, Bitmap.Config.ARGB_8888)
            val random = java.util.Random(123)
            bitmap.setPixels(IntArray(512 * 256) { random.nextInt() or 0xff000000.toInt() }, 0, 512, 0, 0, 512, 256)
            try { File(input.path).outputStream().use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) } }
            finally { bitmap.recycle() }
            val imported = try { container.library.importInput(input, "saf-save.png") { false } }
                finally { container.library.discardInput(input); input.close() }
            container.library.export(imported.id, ExportChoice.CompatiblePng, false).use { lease ->
                val artifact = container.library.prepareHandoff(lease)
                val gateway = AndroidSaveGateway(context.contentResolver)
                val successful = destination(false)
                gateway.save(successful, artifact) { false }
                val saved = context.contentResolver.openInputStream(successful)?.use { it.readBytes() }
                assertArrayEquals(File(artifact.path).readBytes(), saved)
                gateway.discardCreatedDocument(successful)
                val failed = destination(true)
                try { gateway.save(failed, artifact) { false }; fail("Broken provider accepted output") }
                catch (_: java.io.IOException) { }
                assertDeleted(failed)
                val cancelled = destination(false)
                try { gateway.save(cancelled, artifact) { true }; fail("Cancelled output was written") }
                catch (_: CancellationException) { }
                assertDeleted(cancelled)
                assertTrue(File(artifact.path).isFile)
                var recorded = 0
                var closed = 0
                val accounting = object : DetailRepository by container.library {
                    override suspend fun recordSaved(id: String) { container.library.recordSaved(id); recorded++ }
                }
                val job = SupervisorJob()
                val coordinator = SaveCoordinator(accounting, gateway, CoroutineScope(job + Dispatchers.Main.immediate))
                fun guardedLease() = object : OutputLease { override fun close() { closed++ } }
                try {
                    withContext(Dispatchers.Main) { coordinator.prepare(imported.id, guardedLease(), artifact) }
                    assertNotNull(coordinator.state.value.artifact)
                    withContext(Dispatchers.Main) { coordinator.selected(null) }
                    assertEquals(1, closed); assertEquals(0, recorded)
                    val destination = destination(false)
                    withContext(Dispatchers.Main) {
                        coordinator.prepare(imported.id, guardedLease(), artifact)
                        coordinator.selected(destination)
                    }
                    val completed = coordinator.state.first { it.artifact == null && it.saved }
                    assertNull(completed.error); assertEquals(1, recorded); assertEquals(2, closed)
                    gateway.discardCreatedDocument(destination)
                    val broken = destination(true)
                    withContext(Dispatchers.Main) {
                        coordinator.prepare(imported.id, guardedLease(), artifact)
                        coordinator.selected(broken)
                    }
                    val failure = coordinator.state.first { it.artifact == null && it.error != null }
                    assertEquals("IO", failure.error); assertFalse(failure.saved)
                    assertEquals(1, recorded); assertEquals(3, closed)
                    assertDeleted(broken)
                } finally { job.cancelAndJoin() }
            }
        }
    }
    private fun assertDeleted(uri: Uri) {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val id = android.provider.DocumentsContract.getDocumentId(uri)
        val report = context.contentResolver.query(Uri.parse("content://com.grtsinry43.memedock.sharetest.reports/$id"), null, null, null, null)
        requireNotNull(report).use { assertTrue(it.moveToFirst()); assertEquals("deleted", it.getString(0)) }
    }
    private suspend fun destination(failure: Boolean): Uri {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val helper = "com.grtsinry43.memedock.sharetest"
        val key = "save-${System.nanoTime()}"
        context.startActivity(Intent().setClassName(helper, "$helper.SenderActivity")
            .putExtra("createDocument", true).putExtra("failWrite", failure).putExtra("reportKey", key)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_MULTIPLE_TASK))
        while (true) {
            val uri = context.contentResolver.query(Uri.parse("content://$helper.reports/$key"), null, null, null, null)?.use {
                if (it.moveToFirst() && it.getString(0) == "created") it.getString(2) else null
            }
            if (uri != null) return Uri.parse(uri)
            delay(100)
        }
    }
}
