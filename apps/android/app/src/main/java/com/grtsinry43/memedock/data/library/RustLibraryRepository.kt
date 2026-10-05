package com.grtsinry43.memedock.data.library

import com.grtsinry43.memedock.app.LibrarySession
import com.grtsinry43.memedock.bridge.generated.*
import kotlinx.coroutines.*
import java.util.concurrent.atomic.AtomicBoolean

class RustLibraryRepository(private val session: LibrarySession) : LibraryRepository {
    override val changes = session.changes
    private class Cursor(val native: QueryCursorHandle) : PageCursor {
        override fun close() = native.close()
    }
    private class Input(val native: ImportInputHandle) : ImportSlot {
        override val path = native.path()
        val consumed = AtomicBoolean(false)
        override fun close() = native.close()
    }

    override fun retryOpen() = session.retryOpen()
    override suspend fun thumbnailStates(ids: List<String>): List<ThumbnailUpdate> = withContext(Dispatchers.IO) {
        translate {
            session.library().stickerResources(ids).use { task ->
                try { task.awaitResult().map { resource -> ThumbnailUpdate(resource.asset.hash, state(resource.thumbnailStatus), resource.thumbnailPath, resource.lastError) } }
                catch (cancel: CancellationException) { task.cancel(); throw cancel }
            }
        }
    }
    private fun state(status: com.grtsinry43.memedock.bridge.generated.ThumbnailStatus) = when (status) {
        com.grtsinry43.memedock.bridge.generated.ThumbnailStatus.MISSING -> ThumbnailState.Missing
        com.grtsinry43.memedock.bridge.generated.ThumbnailStatus.GENERATING -> ThumbnailState.Generating
        com.grtsinry43.memedock.bridge.generated.ThumbnailStatus.READY -> ThumbnailState.Ready
        com.grtsinry43.memedock.bridge.generated.ThumbnailStatus.FAILED -> ThumbnailState.Failed
    }
    override suspend fun page(text: String, cursor: PageCursor?): LibraryPage = withContext(Dispatchers.IO) {
        translate {
            val library = session.library()
            val task = library.listStickers(StickerQuery(newRequestId(), text, null, emptyList(), null,
                false, StickerSort.RECENT, 60u, (cursor as? Cursor)?.native))
            task.use {
                try {
                    val page = it.awaitResult()
                    val resources = page.resources.associateBy { resource -> resource.asset.hash }
                    LibraryPage(page.stickers.map { sticker ->
                        val resource = requireNotNull(resources[sticker.id]) { "Missing asset metadata" }
                        LibraryItem(sticker.id, sticker.title, sticker.originalName, resource.asset.animated,
                            resource.asset.mime, resource.asset.width.toInt(), resource.asset.height.toInt(),
                            state(resource.thumbnailStatus), resource.thumbnailPath, resource.lastError)
                    }, page.next?.let(::Cursor))
                } catch (cancel: CancellationException) { it.cancel(); throw cancel }
            }
        }
    }
    override suspend fun thumbnail(id: String): String = withContext(Dispatchers.IO) {
        translate {
            session.library().requestThumbnail(id, Priority.VISIBLE).use {
                try { it.awaitResult().path } catch (cancel: CancellationException) { it.cancel(); throw cancel }
            }
        }
    }
    override suspend fun createInput(): ImportSlot = withContext(Dispatchers.IO) {
        translate { session.library().createImportInput().use { Input(it.awaitResult()) } }
    }
    override suspend fun discardInput(input: ImportSlot) = withContext(Dispatchers.IO) {
        val value = input as Input
        if (value.consumed.compareAndSet(false, true)) {
            session.library().discardImportInput(value.native).use { it.awaitResult() }
        }
    }
    override suspend fun importInput(input: ImportSlot, name: String, cancelled: () -> Boolean): ImportedItem = withContext(Dispatchers.IO) {
        translate {
            val value = input as Input
            val library = session.library()
            // The native call consumes ownership even if admission returns Busy.
            value.consumed.set(true)
            val task = library.importStaged(value.native, ImportOptions(name, null, null))
            task.use {
                // UI cancellation is cooperative. Await the actual result even
                // when cancel reports CommitInProgress, preserving a real commit.
                coroutineScope {
                    val monitor = launch {
                        while (isActive) {
                            if (cancelled()) { task.cancel(); break }
                            delay(25)
                        }
                    }
                    try {
                        val result = task.awaitResult()
                        ImportedItem(result.sticker.id, when (result.status) {
                            ImportStatus.CREATED -> ImportDisposition.Created
                            ImportStatus.REUSED -> ImportDisposition.Reused
                            ImportStatus.RESTORE_REQUIRED -> ImportDisposition.RestoreRequired
                        })
                    } finally { monitor.cancelAndJoin() }
                }
            }
        }
    }
    private suspend fun <T> translate(action: suspend () -> T): T = try { action() }
        catch (error: BridgeException.Failure) { throw LibraryFailure(error.code.name, error) }
}
