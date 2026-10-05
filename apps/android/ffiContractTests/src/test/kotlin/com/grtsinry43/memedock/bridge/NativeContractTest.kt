package com.grtsinry43.memedock.bridge

import com.grtsinry43.memedock.bridge.generated.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test
import java.nio.file.Files
import java.io.File

class NativeContractTest {
    @Test fun realImagesImportDeduplicateAndSurviveReopenAcrossGeneratedBindings() = runBlocking {
        withTimeout(30_000) {
            val directory = Files.createTempDirectory("memedock-import-").toFile()
            val config = LibraryConfiguration(File(directory, "data").path, File(directory, "cache").path,
                File(directory, "share").path, defaultResourceConfiguration())
            val pixels = java.awt.image.BufferedImage(40, 20, java.awt.image.BufferedImage.TYPE_INT_ARGB)
            for (y in 0 until 20) for (x in 0 until 40) pixels.setRGB(x, y, 0x804080C0.toInt())
            try {
                var id = ""
                openLibrary(config).use { library ->
                    try {
                        repeat(2) { index ->
                            library.createImportInput().use { it.awaitResult() }.use { input ->
                                assertTrue(javax.imageio.ImageIO.write(pixels, "png", File(input.path())))
                                val result = library.importStaged(input, ImportOptions("真实猫猫.png", null, null)).use { it.awaitResult() }
                                assertEquals(if (index == 0) ImportStatus.CREATED else ImportStatus.REUSED, result.status)
                                id = result.sticker.id
                                expectCode(ErrorCode.CONFLICT) { library.importStaged(input, ImportOptions("again", null, null)).close() }
                            }
                        }
                        val thumbnail = library.requestThumbnail(id, Priority.VISIBLE).use { it.awaitResult() }
                        assertEquals(40, javax.imageio.ImageIO.read(File(thumbnail.path)).width)
                        val resources = library.stickerResources(listOf(id)).use { it.awaitResult() }
                        assertEquals(ThumbnailStatus.READY, resources.single().thumbnailStatus)
                        assertEquals(1L, library.spaceStatistics().use { it.awaitResult() }.knownAssets)
                        val detail = library.stickerDetail(id).use { it.awaitResult() }
                        assertNotNull(detail.originalPath)
                        assertNull(detail.originalError)
                        library.exportOriginal(id).use { it.awaitResult() }.use { lease ->
                            val output = library.prepareHandoff(lease).use { it.awaitResult() }
                            assertEquals("image/png", output.mime)
                            assertArrayEquals(File(requireNotNull(detail.originalPath)).readBytes(), File(output.path).readBytes())
                            assertEquals(0uL, library.cleanExportArtifacts().use { it.awaitResult() })
                            library.exportOriginal(id).use { it.awaitResult() }.use { second -> assertEquals(output.id, second.metadata().id) }
                        }
                    } finally { library.shutdown() }
                }
                openLibrary(config).use { library ->
                    try {
                        val page = library.listStickers(query().copy(text = "真实猫猫")).use { it.awaitResult() }
                        assertEquals(id, page.stickers.single().id)
                        assertNotNull(page.resources.single().thumbnailPath)
                    } finally { library.shutdown() }
                }
            } finally { assertTrue(directory.deleteRecursively()) }
        }
    }

    private fun query(cursor: QueryCursorHandle? = null) = StickerQuery(
        newRequestId(), "", null, emptyList(), null, false, StickerSort.RECENT, 1u, cursor,
    )

    private suspend fun expectCode(code: ErrorCode, action: suspend () -> Unit) {
        try {
            action()
            fail("Expected $code")
        } catch (failure: BridgeException.Failure) {
            assertEquals(code, failure.code)
            assertTrue(failure.detail.isNotEmpty())
        }
    }

    @Test fun persistedMetadataTasksCursorsAndErrorsCrossTheNativeBoundary() = runBlocking {
        withTimeout(30_000) {
            val directory = Files.createTempDirectory("memedock-ffi-").toFile()
            try {
                val data = File(directory, "library")
                val process = ProcessBuilder(System.getProperty("memedock.fixtureBinary"), data.path)
                    .redirectErrorStream(true).start()
                val output = process.inputStream.bufferedReader().use { it.readText() }
                assertEquals(output, 0, process.waitFor())
                val config = LibraryConfiguration(data.path, File(directory, "cache").path,
                    File(directory, "exports").path, defaultResourceConfiguration())
                val library = openLibrary(config)
                val identity = library.identity()
                try {
                    expectCode(ErrorCode.ALREADY_OPEN) { openLibrary(config).close() }
                    val request = query()
                    val page = library.listStickers(request).use { it.awaitResult() }
                    assertEquals(request.requestId, page.requestId)
                    assertEquals(1, page.stickers.size)
                    assertTrue(page.stickers.single().title.startsWith("测试猫猫"))
                    val cursor = requireNotNull(page.next)
                    try {
                        val next = library.listStickers(query(cursor)).use { it.awaitResult() }
                        assertEquals(1, next.stickers.size)
                        assertNotEquals(page.stickers.single().id, next.stickers.single().id)
                        assertNull(next.next)
                        expectCode(ErrorCode.INVALID_INPUT) {
                            library.listStickers(query(cursor).copy(text = "different")).use { it.awaitResult() }
                        }
                        val otherConfig = config.copy(dataDir = File(directory, "other").path,
                            cacheDir = File(directory, "other-cache").path,
                            exportDir = File(directory, "other-export").path)
                        openLibrary(otherConfig).use { other ->
                            try { expectCode(ErrorCode.INVALID_INPUT) { other.listStickers(query(cursor)).close() } }
                            finally { other.shutdown() }
                        }
                    } finally { cursor.close() }
                    val sticker = page.stickers.single()
                    val detail = library.stickerDetail(sticker.id).use { it.awaitResult() }
                    assertEquals(sticker, detail.sticker)
                    assertEquals(10u, detail.asset.width)
                    assertEquals(20u, detail.asset.height)
                    assertTrue(detail.tags.isEmpty())
                    assertTrue(library.collections(false).use { it.awaitResult() }.isEmpty())
                    assertTrue(library.tags(false).use { it.awaitResult() }.isEmpty())
                    assertEquals(2L, library.spaceStatistics().use { it.awaitResult() }.knownAssets)
                    library.verifyOriginal(detail.asset.hash, Priority.BACKGROUND).use { task ->
                        val progress = task.progress()
                        try {
                            assertEquals(detail.asset.hash, task.awaitResult().hash)
                            assertEquals(CancelResult.FINISHED, task.cancel())
                            assertEquals(TaskStatus.Succeeded, task.snapshot().status)
                            expectCode(ErrorCode.CONFLICT) { task.awaitResult() }
                            var terminal = false
                            while (true) {
                                val snapshot = progress.next() ?: break
                                if (snapshot.status == TaskStatus.Succeeded) terminal = true
                            }
                            assertTrue(terminal)
                        } finally { progress.unsubscribe(); progress.close() }
                    }
                    library.subscribe().use { subscription ->
                        // Cancelling a Kotlin await must return the native receiver lease.
                        val waiting = launch(start = CoroutineStart.UNDISPATCHED) { subscription.next() }
                        yield()
                        waiting.cancelAndJoin()
                        val usage = library.recordUse(sticker.id, UsageAction.SHARE_LAUNCHED).use { it.awaitResult() }
                        assertEquals(1L, usage.useCount)
                        assertEquals(sticker.id, (subscription.next() as Notification.UsageChanged).stickerId)
                        val pending = async(start = CoroutineStart.UNDISPATCHED) { subscription.next() }
                        yield()
                        subscription.unsubscribe()
                        assertEquals(Notification.Closed, pending.await())
                    }
                    val path = library.createCheckpoint().use { it.awaitResult() }
                    assertTrue(File(path).isFile)
                    expectCode(ErrorCode.INVALID_INPUT) { library.stickerDetail("bad-id").close() }
                    library.subscribe().use { subscription ->
                        library.shutdown()
                        assertEquals(Notification.Closed, subscription.next())
                    }
                    library.shutdown()
                    assertEquals(LibraryState.CLOSED, library.state())
                    expectCode(ErrorCode.CLOSED) { library.spaceStatistics().close() }
                } finally { library.shutdown(); library.close() }
                openLibrary(config).use { reopened ->
                    try {
                        assertEquals(identity, reopened.identity())
                        assertEquals(2L, reopened.spaceStatistics().use { it.awaitResult() }.knownAssets)
                    } finally { reopened.shutdown() }
                }
            } finally { assertTrue(directory.deleteRecursively()) }
        }
    }

    @Test fun flowCancellationReleasesNativeSubscriptionCapacity() = runBlocking {
        withTimeout(30_000) {
            val directory = Files.createTempDirectory("memedock-flow-").toFile()
            val config = LibraryConfiguration(File(directory, "data").path,
                File(directory, "cache").path, File(directory, "exports").path,
                defaultResourceConfiguration().copy(eventCapacity = 1u))
            try {
                openLibrary(config).use { library ->
                    try {
                        repeat(3) {
                            val subscription = library.subscribe()
                            val collection = launch(start = CoroutineStart.UNDISPATCHED) { subscription.asFlow().first() }
                            yield()
                            collection.cancelAndJoin()
                        }
                    } finally { library.shutdown() }
                }
            } finally { assertTrue(directory.deleteRecursively()) }
        }
    }
}
