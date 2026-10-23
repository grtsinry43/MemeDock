package com.grtsinry43.memedock.data.library

import com.grtsinry43.memedock.app.LibraryBinding
import com.grtsinry43.memedock.app.LibrarySession
import com.grtsinry43.memedock.bridge.generated.*
import kotlinx.coroutines.*

class RustTrashRepository(private val session: LibrarySession) : TrashRepository {
    override val bindings = session.bindings
    override suspend fun binding(): LibraryBinding {
        session.library()
        return session.currentBinding() ?: throw LibraryFailure("CLOSED")
    }
    private suspend fun library(binding: LibraryBinding): LibraryHandle {
        val library = session.library()
        if (session.currentBinding() != binding) throw LibraryFailure("CONFLICT")
        return library
    }
    override suspend fun snapshot(binding: LibraryBinding): List<TrashEntry> = withContext(Dispatchers.IO) {
        translate {
            library(binding).trashSnapshot().use { task ->
                try {
                    task.awaitResult().map { value -> TrashEntry(when (value.kind) {
                        TrashEntityKind.STICKER -> TrashKind.Sticker
                        TrashEntityKind.COLLECTION -> TrashKind.Collection
                        TrashEntityKind.TAG -> TrashKind.Tag
                    }, value.target.id, value.target.generation, value.deletedRevision) }
                } catch (cancel: CancellationException) {
                    task.cancel()
                    withContext(NonCancellable) { task.progress().use { while (it.next() != null) {} } }
                    throw cancel
                }
            }
        }
    }
    override suspend fun purge(binding: LibraryBinding, entry: TrashEntry) = run {
        val library = library(binding)
        val target = EntityReference(entry.id, entry.generation)
        when (entry.kind) {
            TrashKind.Sticker -> library.purgeSticker(target, entry.deletedRevision)
            TrashKind.Collection -> library.purgeCollection(target, entry.deletedRevision)
            TrashKind.Tag -> library.purgeTag(target, entry.deletedRevision)
        }
    }
    override suspend fun cleanup(binding: LibraryBinding) = run { library(binding).retryPurgeCleanup() }
    private suspend fun run(create: suspend () -> PurgeTask): Unit = withContext(Dispatchers.IO) {
        translate {
            create().use { task ->
                try { task.awaitResult() }
                catch (cancel: CancellationException) {
                    task.cancel()
                    withContext(NonCancellable) { task.progress().use { while (it.next() != null) {} } }
                    throw cancel
                }
            }
        }
    }
    private suspend fun <T> translate(block: suspend () -> T): T = try { block() }
    catch (error: BridgeException.Failure) { throw LibraryFailure(error.code.name) }
}
