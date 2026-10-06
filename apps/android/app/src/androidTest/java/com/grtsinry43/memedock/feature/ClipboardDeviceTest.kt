package com.grtsinry43.memedock.feature

import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import com.grtsinry43.memedock.MainActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import com.grtsinry43.memedock.data.settings.ExportChoice
import kotlinx.coroutines.*
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.security.MessageDigest

class ClipboardDeviceTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()
    @Test fun anotherForegroundAppReadsCopiedPngAfterLeaseRelease() = runBlocking {
        withTimeout(30_000) {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val container = (context.applicationContext as MemeDockApplication).container
            val input = container.library.createInput()
            val bitmap = Bitmap.createBitmap(80, 40, Bitmap.Config.ARGB_8888).apply { eraseColor(0x804080C0.toInt()) }
            try { File(input.path).outputStream().use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) } }
            finally { bitmap.recycle() }
            val imported = try { container.library.importInput(input, "clipboard-native.png") { false } }
                finally { container.library.discardInput(input); input.close() }
            val lease = container.library.export(imported.id, ExportChoice.CompatiblePng, false)
            val artifact = container.library.prepareHandoff(lease)
            try {
                compose.waitUntil(10_000) { compose.activity.hasWindowFocus() }
                withContext(Dispatchers.Main) { container.clipboard.copy(lease, artifact) }
            } finally { lease.close() }
            val expected = MessageDigest.getInstance("SHA-256").digest(File(artifact.path).readBytes()).joinToString("") { "%02x".format(it) }
            val helper = "com.grtsinry43.memedock.sharetest"
            val key = "clipboard-${System.nanoTime()}"
            context.startActivity(Intent().setClassName(helper, "$helper.ReceiverActivity")
                .putExtra("clipboard", true).putExtra("reportKey", key)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_MULTIPLE_TASK))
            var report: List<String>? = null
            while (report == null) {
                report = context.contentResolver.query(Uri.parse("content://$helper.reports/$key"), null, null, null, null)?.use {
                    if (it.moveToFirst() && it.getString(0) != "pending") listOf(it.getString(0), it.getLong(1).toString(), it.getString(2)) else null
                }
                if (report == null) delay(100)
            }
            assertEquals(listOf("read", artifact.byteSize.toString(), expected), report)
            assertTrue(File(artifact.path).isFile)
        }
    }
}
