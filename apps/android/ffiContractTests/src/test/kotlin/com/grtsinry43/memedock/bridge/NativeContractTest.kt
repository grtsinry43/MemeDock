package com.grtsinry43.memedock.bridge

import com.grtsinry43.memedock.bridge.generated.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test
import java.nio.file.Files
import java.io.File

class NativeContractTest {
    @Test fun telegramInputsFailAcrossBindingsWithoutLeakingCredentials() = runBlocking {
        withTimeout(30_000) {
            val root = Files.createTempDirectory("memedock-telegram-contract-").toFile()
            val config = LibraryConfiguration(File(root, "data").path, File(root, "cache").path,
                File(root, "share").path, defaultResourceConfiguration())
            try {
                openLibrary(config).use { library ->
                    try {
                        expectCode(ErrorCode.INVALID_INPUT) {
                            library.telegramPack("bad", "https://example.com/addstickers/test").close()
                        }
                        library.telegramPack("invalid-secret-token", "test_pack").use { task ->
                            try {
                                task.awaitResult().close()
                                fail("Expected invalid token")
                            } catch (error: BridgeException.Failure) {
                                assertEquals(ErrorCode.INVALID_INPUT, error.code)
                                assertFalse(error.detail.contains("invalid-secret-token"))
                            }
                        }
                        assertTrue(library.collections(false).use { it.awaitResult() }.isEmpty())
                    } finally { library.shutdown() }
                }
            } finally { assertTrue(root.deleteRecursively()) }
        }
    }
    @Test fun derivedPresetsAndDurableClipboardTokensCrossGeneratedBindings() = runBlocking {
        withTimeout(30_000) {
            val root = Files.createTempDirectory("memedock-export-contract-").toFile()
            val config = LibraryConfiguration(File(root, "data").path, File(root, "cache").path,
                File(root, "share").path, defaultResourceConfiguration())
            var token: String? = null
            try {
                openLibrary(config).use { library ->
                    try {
                        val pixels = java.awt.image.BufferedImage(40, 20, java.awt.image.BufferedImage.TYPE_INT_ARGB)
                        for (y in 0 until 20) for (x in 0 until 40) pixels.setRGB(x, y, 0x804080C0.toInt())
                        val id = library.createImportInput().use { it.awaitResult() }.use { input ->
                            assertTrue(javax.imageio.ImageIO.write(pixels, "png", File(input.path())))
                            library.importStaged(input, ImportOptions("contract.png", null, null)).use { it.awaitResult().sticker.id }
                        }
                        val ids = mutableSetOf<String>()
                        for (preset in listOf(ExportPreset.COMPATIBLE_PNG, ExportPreset.WHITE_BACKGROUND, ExportPreset.SMALL_JPEG)) {
                            val options = ExportOptions(preset, if (preset == ExportPreset.SMALL_JPEG) 512u else 1024u,
                                if (preset == ExportPreset.COMPATIBLE_PNG) null else UInt.MAX_VALUE, true, AnimationPolicy.PRESERVE)
                            library.export(id, options).use { it.awaitResult() }.use { lease ->
                                val artifact = library.prepareHandoff(lease).use { it.awaitResult() }
                                assertTrue(ids.add(artifact.id))
                                val display = javax.imageio.ImageIO.read(File(artifact.path))
                                assertEquals(40, display.width); assertEquals(20, display.height)
                                assertEquals(if (preset == ExportPreset.COMPATIBLE_PNG) 128 else 255, display.getRGB(10, 10) ushr 24)
                                token = library.protectClipboard(lease).use { it.awaitResult() }
                                library.reconcileClipboard(token).use { it.awaitResult() }
                                assertEquals(artifact.id, library.export(id, options).use { it.awaitResult() }.use { it.metadata().id })
                            }
                        }
                        expectCode(ErrorCode.INVALID_INPUT) {
                            library.export(id, ExportOptions(ExportPreset.ORIGINAL, 512u, null, false, AnimationPolicy.PRESERVE)).close()
                        }
                    } finally { library.shutdown() }
                }
                openLibrary(config).use { library ->
                    try {
                        library.reconcileClipboard(token).use { it.awaitResult() }
                        library.reconcileClipboard(null).use { it.awaitResult() }
                    } finally { library.shutdown() }
                }
            } finally { assertTrue(root.deleteRecursively()) }
        }
    }
    @Test fun editingRelationsAndExplicitRecoverySurviveNativeReopen() = runBlocking {
        withTimeout(30_000) {
            val root = Files.createTempDirectory("memedock-management-").toFile()
            val config = LibraryConfiguration(File(root, "data").path, File(root, "cache").path,
                File(root, "share").path, defaultResourceConfiguration())
            var id = ""
            try {
                openLibrary(config).use { library ->
                    try {
                        val collection = library.createCollection("猫猫合集", null).use { it.awaitResult() }
                        val tag = library.createTag("工作").use { it.awaitResult() }
                        library.createImportInput().use { it.awaitResult() }.use { input ->
                            val pixels = java.awt.image.BufferedImage(12, 12, java.awt.image.BufferedImage.TYPE_INT_ARGB)
                            assertTrue(javax.imageio.ImageIO.write(pixels, "png", File(input.path())))
                            id = library.importStaged(input, ImportOptions("cat.png", null, collection.id)).use { it.awaitResult().sticker.id }
                        }
                        val reference = EntityReference(id, 0)
                        library.setStickerOrganization(reference, false, null, listOf(EntityReference(tag.id, tag.lifecycle.generation))).use { it.awaitResult() }
                        library.patchSticker(reference, StickerEdit("摸鱼猫", "明天再说", true)).use { it.awaitResult() }
                        assertEquals(id, library.listStickers(query().copy(text = "工作 明天", starred = true)).use { it.awaitResult().stickers.single().id })
                        library.patchSticker(reference, StickerEdit(null, "", null)).use { it.awaitResult() }
                        val edited = library.stickerDetail(id).use { it.awaitResult() }
                        assertEquals("摸鱼猫", edited.sticker.title)
                        assertEquals("", edited.sticker.note)
                        assertTrue(edited.sticker.starred)
                        assertEquals(collection.id, edited.collection!!.id)
                        val batchTarget = BatchTarget(reference, null)
                        library.batch(listOf(batchTarget, batchTarget), BatchAction.Assign(EntityReference(collection.id, collection.lifecycle.generation))).use {
                            val result = it.awaitResult()
                            assertEquals(1, result.items.size)
                            assertEquals(BatchOutcome.UNCHANGED, result.items.single().outcome)
                        }
                        val extraTag = library.createTag("生活").use { it.awaitResult() }
                        library.batch(listOf(batchTarget), BatchAction.AddTags(listOf(EntityReference(extraTag.id, extraTag.lifecycle.generation)))).use {
                            assertEquals(BatchOutcome.APPLIED, it.awaitResult().items.single().outcome)
                        }
                        assertEquals(setOf(tag.id, extraTag.id), library.stickerDetail(id).use { it.awaitResult().tags.map { tag -> tag.id }.toSet() })
                        library.batch(listOf(batchTarget), BatchAction.RemoveTags(listOf(EntityReference(extraTag.id, extraTag.lifecycle.generation)))).use { it.awaitResult() }
                        val summary = library.collectionSummaries().use { it.awaitResult().single() }
                        assertEquals(1uL, summary.count)
                        assertEquals(id, summary.cover)
                        library.deleteSticker(reference).use { it.awaitResult() }
                        assertTrue(library.listStickers(query()).use { it.awaitResult().stickers.isEmpty() })
                        assertEquals(id, library.listStickers(query().copy(deleted = true)).use { it.awaitResult().stickers.single().id })
                        val suggestions = library.restoreSuggestions(id).use { it.awaitResult() }
                        assertEquals(collection.id, suggestions.collection!!.id)
                        assertEquals(tag.id, suggestions.tags.single().id)
                        val restored = library.restoreSticker(reference, 0).use { it.awaitResult() }
                        assertEquals(1L, restored.lifecycle.generation)
                        val detail = library.stickerDetail(id).use { it.awaitResult() }
                        assertTrue(detail.tags.isEmpty()); assertTrue(detail.collection == null)
                        expectCode(ErrorCode.CONFLICT) { library.patchSticker(reference, StickerEdit("stale", null, null)).use { it.awaitResult() } }
                    } finally { library.shutdown() }
                }
                openLibrary(config).use { library ->
                    try {
                        val detail = library.stickerDetail(id).use { it.awaitResult() }
                        assertEquals("摸鱼猫", detail.sticker.title)
                        assertEquals(1L, detail.sticker.lifecycle.generation)
                        assertTrue(detail.sticker.starred)
                        assertNotNull(detail.originalPath)
                    } finally { library.shutdown() }
                }
            } finally { assertTrue(root.deleteRecursively()) }
        }
    }
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
