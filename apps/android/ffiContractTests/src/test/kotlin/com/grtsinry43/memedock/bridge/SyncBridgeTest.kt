package com.grtsinry43.memedock.bridge

import com.grtsinry43.memedock.bridge.generated.*
import kotlinx.coroutines.*
import org.junit.Assert.*
import org.junit.Test
import java.io.File
import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.nio.file.Files
import java.time.Duration

/** Uses the real Axum test fixture and two real native libraries through generated Kotlin. */
class SyncBridgeTest {
    @Test fun twoLibrariesSyncPurgeAndReimportWithoutRevivingOldGenerations() = runBlocking {
        val fixture = startFixture()
        try {
        withTimeout(120_000) {
            val origin = System.getenv("MEMEDOCK_SYNC_TEST_ORIGIN") ?: "http://127.0.0.1:18787"
            require(URI(origin).host == "127.0.0.1")
            val client = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(5)).build()
            var cookie: String? = null
            fun post(path: String, body: String): String {
                val request = HttpRequest.newBuilder(URI("$origin/api/v1$path"))
                    .timeout(Duration.ofSeconds(10)).header("Origin", origin).header("Content-Type", "application/json")
                    .POST(HttpRequest.BodyPublishers.ofString(body))
                cookie?.let { request.header("Cookie", it) }
                val response = client.send(request.build(), HttpResponse.BodyHandlers.ofString())
                assertEquals(response.body(), 200, response.statusCode())
                response.headers().firstValue("set-cookie").orElse(null)?.let { cookie = it.substringBefore(';') }
                return response.body()
            }
            post("/admin/login", """{"username":"admin","password":"memedock-test-password"}""")
            fun token(name: String): String = Regex("\"token\"\\s*:\\s*\"([^\"]+)\"")
                .find(post("/admin/devices", """{"name":"$name"}"""))?.groupValues?.get(1) ?: error("Missing device token")
            val tokenA = token("Kotlin A")
            val tokenB = token("Kotlin B")
            val root = Files.createTempDirectory("memedock-kotlin-sync").toFile()
            fun configuration(name: String) = LibraryConfiguration(File(root, "$name/data").path,
                File(root, "$name/cache").path, File(root, "$name/share").path, defaultResourceConfiguration())
            suspend fun drain(library: LibraryHandle, credential: String) {
                repeat(32) {
                    val status = library.synchronize(credential).use { it.awaitResult() }
                    assertEquals(0uL, status.blocked)
                    if (status.pending == 0uL) return
                }
                fail("Sync queue did not drain")
            }
            fun png(): ByteArray {
                val image = java.awt.image.BufferedImage(3, 2, java.awt.image.BufferedImage.TYPE_INT_ARGB)
                image.setRGB(1, 1, 0xff43ab76.toInt())
                return java.io.ByteArrayOutputStream().use { output ->
                    check(javax.imageio.ImageIO.write(image, "png", output)); output.toByteArray()
                }
            }
            suspend fun import(library: LibraryHandle, title: String): StickerMetadata =
                library.createImportInput().use { it.awaitResult() }.use { input ->
                    File(input.path()).writeBytes(png())
                    library.importStaged(input, ImportOptions("kotlin.png", title, null)).use { it.awaitResult().sticker }
                }
            try {
                openLibrary(configuration("a")).use { a ->
                    openLibrary(configuration("b")).use { b ->
                        try {
                            val imported = import(a, "Kotlin bridge")
                            a.connectSync(origin, tokenA).use { it.awaitResult() }; drain(a, tokenA)
                            b.connectSync(origin, tokenB).use { it.awaitResult() }; drain(b, tokenB)
                            val detail = b.stickerDetail(imported.id).use { it.awaitResult() }
                            assertEquals("Kotlin bridge", detail.sticker.title)
                            assertNotNull(detail.originalPath)
                            assertArrayEquals(png(), File(detail.originalPath!!).readBytes())
                            val current = a.stickerDetail(imported.id).use { it.awaitResult() }.sticker
                            val reference = EntityReference(current.id, current.lifecycle.generation)
                            val deleted = a.deleteSticker(reference).use { it.awaitResult() }
                            val snapshot = a.trashSnapshot().use { it.awaitResult() }
                            assertTrue(snapshot.any { it.kind == TrashEntityKind.STICKER && it.target == reference })
                            a.purgeSticker(reference, deleted.lifecycle.revision).use { it.awaitResult() }
                            drain(a, tokenA); drain(b, tokenB)
                            assertNotNull(b.stickerDetail(imported.id).use { it.awaitResult() }.sticker.lifecycle.purgedAt)
                            assertFalse(b.trashSnapshot().use { it.awaitResult() }.any { it.target.id == imported.id })
                            val fresh = import(b, "New generation")
                            assertEquals(current.lifecycle.generation + 1, fresh.lifecycle.generation)
                            drain(b, tokenB); drain(a, tokenA)
                            assertEquals("New generation", a.stickerDetail(imported.id).use { it.awaitResult() }.sticker.title)
                            try {
                                a.purgeSticker(reference, deleted.lifecycle.revision).use { it.awaitResult() }
                                fail("Old generation must not delete the reimported sticker")
                            } catch (error: BridgeException.Failure) { assertEquals(ErrorCode.CONFLICT, error.code) }
                            a.retryPurgeCleanup().use { it.awaitResult() }
                            a.subscribeSync().use { subscription ->
                                try {
                                    a.setSyncPaused(true).use { it.awaitResult() }
                                    assertTrue(subscription.next()!!.paused)
                                } finally { subscription.unsubscribe() }
                            }
                        } finally { a.shutdown(); b.shutdown() }
                    }
                }
                openLibrary(configuration("a")).use { reopened ->
                    try {
                        val status = reopened.syncStatus().use { it.awaitResult() }
                        assertTrue(status.connected); assertTrue(status.paused)
                    } finally { reopened.shutdown() }
                }
            } finally { assertTrue(root.deleteRecursively()) }
        }
        } finally { fixture?.close() }
    }

    private suspend fun startFixture(): AutoCloseable? {
        if (System.getenv("MEMEDOCK_SYNC_TEST_ORIGIN") != null) return null
        val repository = File(System.getProperty("memedock.fixtureBinary")).parentFile.parentFile.parentFile
        val log = Files.createTempFile("memedock-sync-server", ".log").toFile()
        val process = ProcessBuilder("cargo", "test", "--locked", "-p", "memedock-server", "--test", "auth_devices",
            "browser_fixture", "--", "--ignored", "--exact", "--nocapture")
            .directory(repository).redirectErrorStream(true).redirectOutput(log).start()
        val cleanup = AutoCloseable {
            process.descendants().forEach { it.destroy() }
            process.destroy()
            if (!process.waitFor(5, java.util.concurrent.TimeUnit.SECONDS)) process.destroyForcibly()
            log.delete()
        }
        try {
            withTimeout(180_000) {
                val client = HttpClient.newBuilder().connectTimeout(Duration.ofMillis(500)).build()
                while (true) {
                    check(process.isAlive) { "Sync server fixture stopped: ${log.readText()}" }
                    val ready = try {
                        client.send(HttpRequest.newBuilder(URI("http://127.0.0.1:18787/api/v1/admin/me"))
                            .timeout(Duration.ofSeconds(1)).GET().build(), HttpResponse.BodyHandlers.discarding()).statusCode() == 401
                    } catch (_: java.io.IOException) { false }
                    if (ready) break
                    delay(100)
                }
            }
            return cleanup
        } catch (error: Throwable) { cleanup.close(); throw error }
    }
}
