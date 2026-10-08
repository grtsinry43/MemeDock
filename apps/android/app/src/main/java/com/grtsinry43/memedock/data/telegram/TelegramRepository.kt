package com.grtsinry43.memedock.data.telegram

enum class TelegramItemState { Available, Imported, RestoreRequired, OriginalMissing }
data class TelegramItem(val id: String, val emoji: String?, val animated: Boolean, val state: TelegramItemState, val hasPreview: Boolean)
interface TelegramPack : AutoCloseable {
    val title: String
    val items: List<TelegramItem>
}
enum class TelegramOutcome { Pending, Created, Reused, RestoreRequired, Failed }
data class TelegramResult(val id: String, val outcome: TelegramOutcome, val error: String? = null)
data class TelegramReport(val items: List<TelegramResult>, val stopped: Boolean) {
    val completed get() = items.count { it.outcome != TelegramOutcome.Pending }
}
interface TelegramRepository {
    suspend fun load(token: String, name: String): TelegramPack
    suspend fun preview(pack: TelegramPack, id: String): String
    suspend fun import(pack: TelegramPack, ids: List<String>, cancelled: () -> Boolean, progress: (TelegramReport) -> Unit): TelegramReport
}
