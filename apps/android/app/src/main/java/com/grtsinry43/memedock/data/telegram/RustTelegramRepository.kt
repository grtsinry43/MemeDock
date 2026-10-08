package com.grtsinry43.memedock.data.telegram

import com.grtsinry43.memedock.app.LibrarySession
import com.grtsinry43.memedock.bridge.generated.*
import com.grtsinry43.memedock.data.library.LibraryFailure
import kotlinx.coroutines.*

class RustTelegramRepository(private val session: LibrarySession) : TelegramRepository {
    private class Pack(val handle: TelegramPackHandle) : TelegramPack {
        private val metadata = handle.metadata()
        override val title = metadata.title
        override val items = metadata.stickers.map {
            TelegramItem(it.id, it.emoji, it.format != TelegramFormat.STATIC, when (it.state) {
                TelegramImportState.AVAILABLE -> TelegramItemState.Available
                TelegramImportState.IMPORTED -> TelegramItemState.Imported
                TelegramImportState.RESTORE_REQUIRED -> TelegramItemState.RestoreRequired
                TelegramImportState.ORIGINAL_MISSING -> TelegramItemState.OriginalMissing
            }, it.hasPreview)
        }
        override fun close() = handle.close()
    }
    override suspend fun load(token: String, name: String): TelegramPack {
        var acquired: TelegramPackHandle? = null
        try {
            val result = withContext(Dispatchers.IO) { translate {
                session.library().telegramPack(token, name).use { task ->
                    try {
                        task.awaitResult().also { acquired = it }.let { Pack(it) }
                    } catch (cancel: CancellationException) {
                        runCatching { task.cancel() }; throw cancel
                    }
                }
            } }
            acquired = null
            return result
        } finally { acquired?.close() }
    }
    override suspend fun preview(pack: TelegramPack, id: String): String = withContext(Dispatchers.IO) { translate {
        session.library().telegramPreview((pack as Pack).handle, id).use { task ->
            try { task.awaitResult() }
            catch (cancel: CancellationException) { runCatching { task.cancel() }; throw cancel }
        }
    } }
    override suspend fun import(pack: TelegramPack, ids: List<String>, cancelled: () -> Boolean,
        progress: (TelegramReport) -> Unit): TelegramReport = withContext(Dispatchers.IO) { translate {
        session.library().importTelegram((pack as Pack).handle, ids).use { task ->
            coroutineScope {
                val monitor = launch {
                    while (isActive) {
                        progress(task.snapshot().model())
                        if (cancelled()) { runCatching { task.cancel() }; break }
                        delay(150)
                    }
                }
                try { task.awaitResult().model() }
                catch (cancel: CancellationException) { runCatching { task.cancel() }; throw cancel }
                finally { monitor.cancelAndJoin() }
            }
        }
    } }
    private suspend fun <T> translate(action: suspend () -> T): T = try { action() }
        catch (error: BridgeException.Failure) { throw LibraryFailure(error.code.name) }
}

private fun TelegramImportReport.model() = TelegramReport(items.map {
    TelegramResult(it.id, when (it.outcome) {
        TelegramImportOutcome.PENDING -> TelegramOutcome.Pending
        TelegramImportOutcome.CREATED -> TelegramOutcome.Created
        TelegramImportOutcome.REUSED -> TelegramOutcome.Reused
        TelegramImportOutcome.RESTORE_REQUIRED -> TelegramOutcome.RestoreRequired
        TelegramImportOutcome.FAILED -> TelegramOutcome.Failed
    }, it.error?.name)
}, stopped)
