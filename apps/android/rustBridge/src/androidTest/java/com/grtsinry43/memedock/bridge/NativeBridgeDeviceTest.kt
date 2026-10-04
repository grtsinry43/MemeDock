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
