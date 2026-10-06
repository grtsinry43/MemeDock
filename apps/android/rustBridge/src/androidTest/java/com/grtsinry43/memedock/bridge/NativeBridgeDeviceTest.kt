package com.grtsinry43.memedock.bridge

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.grtsinry43.memedock.bridge.generated.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.UUID

@RunWith(AndroidJUnit4::class)
class NativeBridgeDeviceTest {
    @Test fun nativeDerivedExportAndClipboardProtectionSurviveReopenOnDevice() = runBlocking {
        withTimeout(30_000) {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val directory = File(context.filesDir, "bridge-export-${UUID.randomUUID()}")
            val config = LibraryConfiguration(File(directory, "data").path,
                File(directory, "cache").path, File(directory, "exports").path, defaultResourceConfiguration())
            var token: String? = null
            try {
                openLibrary(config).use { library ->
                    try {
                        val id = library.createImportInput().use { it.awaitResult() }.use { input ->
                            val bitmap = android.graphics.Bitmap.createBitmap(40, 20, android.graphics.Bitmap.Config.ARGB_8888)
                            try {
                                bitmap.eraseColor(0x804080C0.toInt())
                                File(input.path()).outputStream().use { assertTrue(bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)) }
                            } finally { bitmap.recycle() }
                            library.importStaged(input, ImportOptions("native-export.png", null, null)).use { it.awaitResult().sticker.id }
                        }
                        library.export(id, ExportOptions(ExportPreset.WHITE_BACKGROUND, null, UInt.MAX_VALUE, true, AnimationPolicy.PRESERVE))
                            .use { it.awaitResult() }.use { lease ->
                                val artifact = library.prepareHandoff(lease).use { it.awaitResult() }
                                assertEquals("image/png", artifact.mime)
                                val bitmap = android.graphics.BitmapFactory.decodeFile(artifact.path)
                                try { assertEquals(40, bitmap.width); assertEquals(255, bitmap.getPixel(10, 10) ushr 24) }
                                finally { bitmap.recycle() }
                                token = library.protectClipboard(lease).use { it.awaitResult() }
                            }
                    } finally { library.shutdown() }
                }
                openLibrary(config).use { library ->
                    try {
                        library.reconcileClipboard(token).use { it.awaitResult() }
                        assertEquals(0uL, library.cleanExportArtifacts().use { it.awaitResult() })
                        library.reconcileClipboard(null).use { it.awaitResult() }
                    } finally { library.shutdown() }
                }
            } finally { assertTrue(directory.deleteRecursively()) }
        }
    }
    @Test fun nativeLibraryOpensQueriesClosesAndReopensOnDevice() = runBlocking {
        withTimeout(30_000) {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val directory = File(context.filesDir, "bridge-test-${UUID.randomUUID()}")
            val config = LibraryConfiguration(File(directory, "data").path,
                File(directory, "cache").path, File(directory, "exports").path,
                defaultResourceConfiguration().copy(eventCapacity = 1u))
            try {
                val library = openLibrary(config)
                val identity = library.identity()
                try {
                    assertEquals(LibraryState.OPEN, library.state())
                    assertEquals(0L, library.spaceStatistics().use { it.awaitResult() }.knownAssets)
                    val request = StickerQuery(newRequestId(), "", null, emptyList(), null,
                        false, StickerSort.RECENT, 10u, null)
                    val page = library.listStickers(request).use { it.awaitResult() }
                    assertEquals(request.requestId, page.requestId)
                    assertTrue(page.stickers.isEmpty())
                    try { library.stickerDetail("invalid").close(); fail("Expected validation error") }
                    catch (error: BridgeException.Failure) { assertEquals(ErrorCode.INVALID_INPUT, error.code) }
                    val collector = launch(start = CoroutineStart.UNDISPATCHED) { library.subscribe().asFlow().first() }
                    yield()
                    collector.cancelAndJoin()
                    library.subscribe().use { subscription ->
                        val closing = async(start = CoroutineStart.UNDISPATCHED) { subscription.next() }
                        library.shutdown()
                        assertEquals(Notification.Closed, closing.await())
                    }
                    assertEquals(LibraryState.CLOSED, library.state())
                } finally { library.shutdown(); library.close() }
                openLibrary(config).use { reopened ->
                    try { assertEquals(identity, reopened.identity()) }
                    finally { reopened.shutdown() }
                }
            } finally { assertTrue(directory.deleteRecursively()) }
        }
    }
}
