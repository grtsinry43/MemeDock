package com.grtsinry43.memedock.data.library

import com.grtsinry43.memedock.app.LibrarySession
import com.grtsinry43.memedock.bridge.generated.*
import kotlinx.coroutines.*
import java.util.concurrent.atomic.AtomicBoolean

class RustLibraryRepository(private val session: LibrarySession) : LibraryRepository, DetailRepository, CollectionRepository, ManagementRepository {
    override val changes = session.changes
    private class Cursor(val native: QueryCursorHandle) : PageCursor {
        override fun close() = native.close()
    }
    private class Input(val native: ImportInputHandle) : ImportSlot {
        override val path = native.path()
        val consumed = AtomicBoolean(false)
        override fun close() = native.close()
    }
    private class Lease(val native: ArtifactLeaseHandle) : OutputLease {
        override fun close() = native.close()
    }
    override suspend fun detail(id: String): StickerDetails = withContext(Dispatchers.IO) {
        translate {
            session.library().stickerDetail(id).use { task ->
                try {
                    val value = task.awaitResult()
                    val preview = if (value.asset.animated && value.asset.mime == "image/png" && value.originalPath != null) thumbnail(id) else value.originalPath
                    StickerDetails(id, value.sticker.title, value.sticker.note, value.sticker.originalName,
                        value.asset.mime, value.asset.width.toInt(), value.asset.height.toInt(), value.asset.byteSize,
                        value.asset.animated, value.sticker.lifecycle.deletedAt != null, value.tags.map { it.model() },
                        value.collections.map { it.model() }, preview, value.originalError?.name,
                        value.sticker.starred, value.sticker.lifecycle.generation, value.sticker.lifecycle.revision)
                } catch (cancel: CancellationException) { task.cancel(); throw cancel }
            }
        }
    }
    override suspend fun exportOriginal(id: String): OutputLease {
        var acquired: Lease? = null
        try {
        val result = withContext(Dispatchers.IO) { translate {
            session.library().exportOriginal(id).use { task ->
                try { Lease(task.awaitResult()).also { acquired = it } } catch (cancel: CancellationException) { task.cancel(); throw cancel }
            }
        } }
        acquired = null
        return result
        } finally { acquired?.close() }
    }
    override suspend fun prepareHandoff(lease: OutputLease): ShareArtifact = withContext(Dispatchers.IO) {
        translate {
            session.library().prepareHandoff((lease as Lease).native).use { task ->
                try { task.awaitResult().let { ShareArtifact(it.id, it.path, it.mime, it.fileName, it.byteSize.toLong(), it.animated) } }
                catch (cancel: CancellationException) { task.cancel(); throw cancel }
            }
        }
    }
    override suspend fun recordShareLaunched(id: String) = withContext(Dispatchers.IO) {
        translate { session.library().recordUse(id, UsageAction.SHARE_LAUNCHED).use { it.awaitResult(); Unit } }
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
    override suspend fun page(text: String, cursor: PageCursor?, collectionId: String?, starred: Boolean?, deleted: Boolean): LibraryPage {
        var acquired: Cursor? = null
        try {
        val result = withContext(Dispatchers.IO) {
        translate {
            val library = session.library()
            val task = library.listStickers(StickerQuery(newRequestId(), text, collectionId, emptyList(), starred,
                deleted, if (collectionId == null) StickerSort.RECENT else StickerSort.COLLECTION_ORDER, 60u, (cursor as? Cursor)?.native))
            task.use {
                try {
                    val page = it.awaitResult()
                    val resources = page.resources.associateBy { resource -> resource.asset.hash }
                    LibraryPage(page.stickers.map { sticker ->
                        val resource = requireNotNull(resources[sticker.id]) { "Missing asset metadata" }
                        LibraryItem(sticker.id, sticker.title, sticker.originalName, resource.asset.animated,
                            resource.asset.mime, resource.asset.width.toInt(), resource.asset.height.toInt(),
                            state(resource.thumbnailStatus), resource.thumbnailPath, resource.lastError,
                            sticker.starred, sticker.lifecycle.generation, sticker.lifecycle.revision, sticker.lifecycle.deletedAt != null)
                    }, page.next?.let { native -> Cursor(native).also { acquired = it } })
                } catch (cancel: CancellationException) { it.cancel(); throw cancel }
            }
        }
        }
        acquired = null
        return result
        } finally { acquired?.close() }
    }
    override suspend fun thumbnail(id: String): String = withContext(Dispatchers.IO) {
        translate {
            session.library().requestThumbnail(id, Priority.VISIBLE).use {
                try { it.awaitResult().path } catch (cancel: CancellationException) { it.cancel(); throw cancel }
            }
        }
    }
    override suspend fun createInput(): ImportSlot {
        var acquired: Input? = null
        try {
            val result = withContext(Dispatchers.IO) {
                translate { session.library().createImportInput().use { task ->
                    try { Input(task.awaitResult()).also { acquired = it } }
                    catch (cancel: CancellationException) { task.cancel(); throw cancel }
                } }
            }
            acquired = null
            return result
        } catch (error: Exception) {
            acquired?.let { input ->
                try { withContext(NonCancellable) { discardInput(input) } }
                catch (cleanup: Exception) { error.addSuppressed(cleanup) }
                finally { input.close() }
            }
            throw error
        }
    }
    override suspend fun discardInput(input: ImportSlot) = withContext(Dispatchers.IO) {
        val value = input as Input
        if (value.consumed.compareAndSet(false, true)) {
            session.library().discardImportInput(value.native).use { it.awaitResult() }
        }
    }
    override suspend fun importInput(input: ImportSlot, name: String, collectionId: String?, cancelled: () -> Boolean): ImportedItem = withContext(Dispatchers.IO) {
        translate {
            val value = input as Input
            val library = session.library()
            // The native call consumes ownership even if admission returns Busy.
            value.consumed.set(true)
            val task = library.importStaged(value.native, ImportOptions(name, null, collectionId))
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

    override suspend fun collections(): List<LibraryCollection> = collections(false)
    override suspend fun collections(deleted: Boolean): List<LibraryCollection> = withContext(Dispatchers.IO) { translate {
        session.library().collections(deleted).use { task ->
            try { task.awaitResult().map { it.model() } }
            catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    private fun CollectionMetadata.model() = LibraryCollection(id, name, lifecycle.generation, lifecycle.revision, lifecycle.deletedAt)
    private fun TagMetadata.model() = LibraryTag(id, name, lifecycle.generation, lifecycle.revision, lifecycle.deletedAt)
    private fun StickerDetails.reference() = EntityReference(id, generation)
    private fun LibraryCollection.reference() = EntityReference(id, generation)
    private fun LibraryTag.reference() = EntityReference(id, generation)
    override suspend fun patchSticker(detail: StickerDetails, title: String?, note: String?, starred: Boolean?): Unit = withContext(Dispatchers.IO) { translate {
        session.library().patchSticker(detail.reference(), StickerEdit(title, note, starred)).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun relations(detail: StickerDetails, collections: List<LibraryCollection>?, tags: List<LibraryTag>?): Unit = withContext(Dispatchers.IO) { translate {
        session.library().setStickerRelations(detail.reference(), collections?.map { it.reference() }, tags?.map { it.reference() }).use { task ->
            try { task.awaitResult() } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun createCollection(name: String): LibraryCollection = withContext(Dispatchers.IO) { translate {
        session.library().createCollection(name, null).use { task ->
            try { task.awaitResult().model() } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun renameCollection(value: LibraryCollection, name: String): Unit = withContext(Dispatchers.IO) { translate {
        session.library().renameCollection(value.reference(), name).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun deleteCollection(value: LibraryCollection): Unit = withContext(Dispatchers.IO) { translate {
        session.library().deleteCollection(value.reference()).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun moveCollection(value: LibraryCollection, before: LibraryCollection?): Unit = withContext(Dispatchers.IO) { translate {
        session.library().moveCollection(value.reference(), before?.id).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun moveCollectionItem(collection: LibraryCollection, value: LibraryItem, before: LibraryItem?): Unit = withContext(Dispatchers.IO) { translate {
        session.library().moveCollectionItem(collection.reference(), EntityReference(value.id, value.generation), before?.id).use { task ->
            try { task.awaitResult() } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun tags(deleted: Boolean): List<LibraryTag> = withContext(Dispatchers.IO) { translate {
        session.library().tags(deleted).use { task ->
            try { task.awaitResult().map { it.model() } } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun createTag(name: String): LibraryTag = withContext(Dispatchers.IO) { translate {
        session.library().createTag(name).use { task ->
            try { task.awaitResult().model() } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun renameTag(value: LibraryTag, name: String): Unit = withContext(Dispatchers.IO) { translate {
        session.library().renameTag(value.reference(), name).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun deleteTag(value: LibraryTag): Unit = withContext(Dispatchers.IO) { translate {
        session.library().deleteTag(value.reference()).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun deleteSticker(value: StickerDetails): Unit = withContext(Dispatchers.IO) { translate {
        session.library().deleteSticker(value.reference()).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun restoreSticker(value: StickerDetails): Unit = withContext(Dispatchers.IO) { translate {
        session.library().restoreSticker(value.reference(), value.revision).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun restoreCollection(value: LibraryCollection): Unit = withContext(Dispatchers.IO) { translate {
        session.library().restoreCollection(value.reference(), value.revision).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun restoreTag(value: LibraryTag): Unit = withContext(Dispatchers.IO) { translate {
        session.library().restoreTag(value.reference(), value.revision).use { task ->
            try { task.awaitResult(); Unit } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
    override suspend fun suggestions(id: String): RestoreSuggestions = withContext(Dispatchers.IO) { translate {
        session.library().restoreSuggestions(id).use { task ->
            try { task.awaitResult().let { RestoreSuggestions(it.collections.map { c -> c.model() }, it.tags.map { t -> t.model() }) } } catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }

    override suspend fun statistics(): LibraryStatistics = withContext(Dispatchers.IO) { translate {
        session.library().spaceStatistics().use { task ->
            try { task.awaitResult().let { LibraryStatistics(it.knownAssets, it.readyOriginalBytes) } }
            catch (cancel: CancellationException) { task.cancel(); throw cancel }
        }
    } }
}
