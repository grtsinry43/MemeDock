package com.grtsinry43.memedock.feature

import android.content.ContentValues
import android.graphics.Bitmap
import android.provider.MediaStore
import androidx.test.platform.app.InstrumentationRegistry
import com.grtsinry43.memedock.bridge.generated.*
import com.grtsinry43.memedock.platform.importing.AndroidImportGateway
import com.grtsinry43.memedock.data.library.LibraryFailure
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.*
import org.junit.Test
import java.io.File

class ImportDeviceTest {
    @Test fun cancellingARealUriCopyLeavesOnlyTheBoundedPartialFileAndAllowsAnotherRead() = runBlocking {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val resolver = context.contentResolver
        val uri = requireNotNull(resolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, ContentValues().apply {
            put(MediaStore.Images.Media.DISPLAY_NAME, "MemeDock-cancel-test.png")
            put(MediaStore.Images.Media.MIME_TYPE, "image/png")
        }))
        val file = File(context.cacheDir, "cancel-test-${System.nanoTime()}.part")
        try {
            val bitmap = Bitmap.createBitmap(512, 512, Bitmap.Config.ARGB_8888)
            val random = java.util.Random(43)
            val colors = IntArray(512 * 512) { random.nextInt() }
            bitmap.setPixels(colors, 0, 512, 0, 0, 512, 512)
            resolver.openOutputStream(uri)!!.use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
            bitmap.recycle()
            val gateway = AndroidImportGateway(resolver)
            val candidate = gateway.describe(uri.toString())
            var cancelled = false
            try {
                gateway.copy(candidate, file.path, { cancelled }) { cancelled = true }
                fail("Expected cancellation during reading")
            } catch (error: LibraryFailure) { assertEquals("CANCELLED", error.reason) }
            assertEquals(256L * 1024, file.length())
            gateway.copy(candidate, file.path, { false }) {}
            assertTrue(file.length() > 256L * 1024)
        } finally {
            resolver.delete(uri, null, null)
            if (file.exists()) assertTrue(file.delete())
        }
    }

    @Test fun contentUriImportsRealPixelsDeduplicatesAndSurvivesRestart() = runBlocking {
        withTimeout(30_000) {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val resolver = context.contentResolver
            val uri = requireNotNull(resolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, ContentValues().apply {
                put(MediaStore.Images.Media.DISPLAY_NAME, "MemeDock-device-test.png")
                put(MediaStore.Images.Media.MIME_TYPE, "image/png")
            }))
            val root = File(context.cacheDir, "device-import-${System.nanoTime()}")
            val config = LibraryConfiguration(File(root, "data").path, File(root, "cache").path, File(root, "share").path, defaultResourceConfiguration())
            try {
                val bitmap = Bitmap.createBitmap(80, 40, Bitmap.Config.ARGB_8888)
                bitmap.eraseColor(0x804080C0.toInt())
                resolver.openOutputStream(uri)!!.use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
                bitmap.recycle()
                val gateway = AndroidImportGateway(resolver)
                val candidate = gateway.describe(uri.toString())
                assertEquals("MemeDock-device-test.png", candidate.name)
                var id = ""
                openLibrary(config).use { library ->
                    try {
                        repeat(2) { index ->
                            library.createImportInput().use { it.awaitResult() }.use { input ->
                                var read = 0L
                                gateway.copy(candidate, input.path(), { false }) { read = it }
                                assertTrue(read > 0)
                                val result = library.importStaged(input, ImportOptions(candidate.name, null, null)).use { it.awaitResult() }
                                assertEquals(if (index == 0) ImportStatus.CREATED else ImportStatus.REUSED, result.status)
                                id = result.sticker.id
                            }
                        }
                        val thumbnail = library.requestThumbnail(id, Priority.VISIBLE).use { it.awaitResult() }
                        assertTrue(File(thumbnail.path).isFile)
                    } finally { library.shutdown() }
                }
                openLibrary(config).use { library ->
                    try {
                        val page = library.listStickers(StickerQuery(newRequestId(), "MemeDock-device", null, emptyList(), null, false, StickerSort.RECENT, 60u, null)).use { it.awaitResult() }
                        assertEquals(id, page.stickers.single().id)
                        assertEquals(80u, page.resources.single().asset.width)
                        assertNotNull(page.resources.single().thumbnailPath)
                    } finally { library.shutdown() }
                }
                val destination = File(root, "failed.part")
                try {
                    AndroidImportGateway(resolver, maxBytes = 8).copy(candidate, destination.path, { false }) {}
                    fail("Expected resource limit")
                } catch (error: LibraryFailure) { assertEquals("RESOURCE_LIMIT", error.reason) }
                try {
                    gateway.copy(candidate, destination.path, { true }) {}
                    fail("Expected cancellation")
                } catch (error: LibraryFailure) { assertEquals("CANCELLED", error.reason) }
                try {
                    val missing = android.content.ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, Long.MAX_VALUE)
                    gateway.copy(candidate.copy(uri = missing.toString()), destination.path, { false }) {}
                    fail("Expected missing URI failure")
                } catch (_: java.io.FileNotFoundException) { }
                catch (_: SecurityException) { }
            } finally {
                resolver.delete(uri, null, null)
                assertTrue(root.deleteRecursively())
            }
        }
    }
}
