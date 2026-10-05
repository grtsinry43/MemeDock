package com.grtsinry43.memedock.feature

import android.content.Intent
import android.net.Uri
import android.graphics.Bitmap
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.core.content.IntentCompat
import androidx.test.platform.app.InstrumentationRegistry
import com.grtsinry43.memedock.MainActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import com.grtsinry43.memedock.feature.importing.ImportItemStatus
import com.grtsinry43.memedock.feature.importing.ImportPhase
import com.grtsinry43.memedock.platform.importing.ShareIntentParser
import com.grtsinry43.memedock.platform.sharing.AndroidShareGateway
import com.grtsinry43.memedock.data.library.LibraryFailure
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.security.MessageDigest

class ShareDeviceTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()
    private val helper = "com.grtsinry43.memedock.sharetest"

    @Test fun parserRejectsMalformedInputsAndDeduplicatesClipData() {
        val uri = Uri.parse("content://fixture/images/1")
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "image/png"; putExtra(Intent.EXTRA_STREAM, uri)
            clipData = android.content.ClipData.newRawUri("image", uri)
        }
        assertEquals(listOf(uri.toString()), ShareIntentParser.parse(send))
        assertTrue(ShareIntentParser.parse(Intent(Intent.ACTION_MAIN)).isEmpty())
        send.putExtra(Intent.EXTRA_STREAM, Uri.parse("file:///private/image.png"))
        try { ShareIntentParser.parse(send); fail("Unsafe URI accepted") }
        catch (error: LibraryFailure) { assertEquals("INVALID_INPUT", error.reason) }
    }

    @Test fun sharedBatchIsStagedBeforeConfirmationAndImportsAfterSourceGrantRevocation() = runBlocking {
        withTimeout(30_000) {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val imports = (context.applicationContext as MemeDockApplication).container.imports
            withContext(Dispatchers.Main) { if (!imports.state.value.busy) imports.discard() }
            imports.state.first { it.phase == ImportPhase.Finished }
            context.startActivity(Intent().setClassName(helper, "$helper.SenderActivity")
                .putExtra("multiple", true).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_MULTIPLE_TASK))
            val review = try { withTimeout(10_000) { imports.state.first { it.phase == ImportPhase.Review && it.items.size == 2 && it.items.all { item -> item.status == ImportItemStatus.Staged } } } }
                catch (error: TimeoutCancellationException) { throw AssertionError("Unexpected incoming state: ${imports.state.value}", error) }
            assertEquals(2, review.items.size)
            val revoked = "revoked-${System.nanoTime()}"
            context.startActivity(Intent().setClassName(helper, "$helper.SenderActivity")
                .putExtra("revoke", true).putExtra("reportKey", revoked).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_MULTIPLE_TASK))
            assertEquals("revoked", report(context, revoked)[0])
            try { context.contentResolver.openInputStream(Uri.parse(review.items.first().candidate.uri))?.close(); fail("Source grant was not revoked") }
            catch (_: SecurityException) { }
            withContext(Dispatchers.Main) { imports.start() }
            val result = imports.state.first { it.phase == ImportPhase.Finished }
            assertTrue(result.items.all { it.status == ImportItemStatus.Created || it.status == ImportItemStatus.Reused })
            withContext(Dispatchers.Main) { imports.hide() }
        }
    }

    @Test fun exportedOriginalNeedsGrantAndRemainsReadableAfterLeaseReleaseInBackground() = runBlocking {
        withTimeout(30_000) {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val repository = (context.applicationContext as MemeDockApplication).container.library
            val input = repository.createInput()
            val bitmap = Bitmap.createBitmap(80, 40, Bitmap.Config.ARGB_8888).apply { eraseColor(0x804080C0.toInt()) }
            try { File(input.path).outputStream().use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) } }
            finally { bitmap.recycle() }
            val imported = try { repository.importInput(input, "MemeDock-outgoing.png") { false } }
                finally { repository.discardInput(input); input.close() }
            val lease = repository.exportOriginal(imported.id)
            val artifact = repository.prepareHandoff(lease)
            val gateway = AndroidShareGateway()
            val send = gateway.intentFor(context, artifact)
            val uri = requireNotNull(IntentCompat.getParcelableExtra(send, Intent.EXTRA_STREAM, Uri::class.java))
            context.revokeUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            val denied = "denied-${System.nanoTime()}"
            // ACTION_SEND can promote EXTRA_STREAM to ClipData and grant read
            // access itself. A custom explicit probe avoids that OS behavior.
            context.startActivity(Intent("com.grtsinry43.memedock.sharetest.PROBE").setClassName(helper, "$helper.ReceiverActivity")
                .setType(artifact.mime).putExtra(Intent.EXTRA_STREAM, uri).putExtra("reportKey", denied).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_MULTIPLE_TASK))
            assertTrue(report(context, denied)[0] in listOf("SecurityException", "FileNotFoundException"))
            val granted = "granted-${System.nanoTime()}"
            val expected = MessageDigest.getInstance("SHA-256").digest(File(artifact.path).readBytes()).joinToString("") { "%02x".format(it) }
            context.startActivity(send.setClassName(helper, "$helper.ReceiverActivity").putExtra("reportKey", granted).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_MULTIPLE_TASK))
            lease.close()
            val result = report(context, granted)
            assertEquals("read", result[0]); assertEquals(artifact.byteSize.toString(), result[1]); assertEquals(expected, result[2])
            assertTrue(File(artifact.path).isFile)
        }
    }
    private suspend fun report(context: android.content.Context, key: String): List<String> {
        while (true) {
            val value = requireNotNull(context.contentResolver.query(Uri.parse("content://$helper.reports/$key"), null, null, null, null)) { "Test report provider is not visible" }.use {
                if (it.moveToFirst()) listOf(it.getString(0), it.getLong(1).toString(), it.getString(2)) else null
            }
            if (value != null && value[0] != "pending") return value
            delay(100)
        }
    }
}
