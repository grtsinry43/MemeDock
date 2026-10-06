package com.grtsinry43.memedock.sharetest

import android.database.Cursor
import android.database.MatrixCursor
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract.Document
import android.provider.DocumentsContract.Root
import android.provider.DocumentsProvider
import java.io.File
import java.io.FileNotFoundException
import java.util.UUID

/** Real SAF fixture. The failure folder closes its sink during a write. */
class TestDocumentsProvider : DocumentsProvider() {
    private val directory get() = File(requireNotNull(context).filesDir, "documents").apply { mkdirs() }
    private val preferences get() = requireNotNull(context).getSharedPreferences("documents", 0)
    override fun onCreate() = true
    override fun queryRoots(projection: Array<out String>?): Cursor = MatrixCursor(projection ?: arrayOf(
        Root.COLUMN_ROOT_ID, Root.COLUMN_DOCUMENT_ID, Root.COLUMN_TITLE, Root.COLUMN_FLAGS, Root.COLUMN_MIME_TYPES)).apply {
        val values = mapOf<String, Any>(Root.COLUMN_ROOT_ID to "test", Root.COLUMN_DOCUMENT_ID to "root",
            Root.COLUMN_TITLE to "MemeDock 保存验证", Root.COLUMN_FLAGS to Root.FLAG_SUPPORTS_CREATE,
            Root.COLUMN_MIME_TYPES to "image/*")
        addRow(columnNames.map { values[it] }.toTypedArray())
    }
    private fun cursor(projection: Array<out String>?) = MatrixCursor(projection ?: arrayOf(
        Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE,
        Document.COLUMN_FLAGS, Document.COLUMN_SIZE))
    private fun add(cursor: MatrixCursor, id: String) {
        val folder = id in listOf("root", "success", "failure")
        val file = if (folder) null else file(id).takeIf { it.isFile } ?: throw FileNotFoundException(id)
        val values = mapOf<String, Any>(Document.COLUMN_DOCUMENT_ID to id,
            Document.COLUMN_DISPLAY_NAME to when (id) { "root" -> "MemeDock 保存验证"; "success" -> "正常保存";
                "failure" -> "写入失败"; else -> preferences.getString("$id.name", id).orEmpty() },
            Document.COLUMN_MIME_TYPE to if (folder) Document.MIME_TYPE_DIR else preferences.getString("$id.mime", "image/png").orEmpty(),
            Document.COLUMN_FLAGS to if (folder) Document.FLAG_DIR_SUPPORTS_CREATE else Document.FLAG_SUPPORTS_WRITE or Document.FLAG_SUPPORTS_DELETE,
            Document.COLUMN_SIZE to (file?.length() ?: 0L))
        cursor.addRow(cursor.columnNames.map { values[it] }.toTypedArray())
    }
    private fun file(id: String): File {
        val raw = id.removePrefix("fail-")
        if (runCatching { UUID.fromString(raw).toString() == raw }.getOrDefault(false).not()) throw FileNotFoundException(id)
        return File(directory, id)
    }
    override fun queryDocument(documentId: String, projection: Array<out String>?): Cursor = cursor(projection).also { add(it, documentId) }
    override fun queryChildDocuments(parentDocumentId: String, projection: Array<out String>?, sortOrder: String?): Cursor =
        cursor(projection).also { result ->
            if (parentDocumentId == "root") { add(result, "success"); add(result, "failure") }
            else directory.listFiles().orEmpty().filter { it.isFile && preferences.getString("${it.name}.parent", null) == parentDocumentId }
                .forEach { add(result, it.name) }
        }
    override fun createDocument(parentDocumentId: String, mimeType: String, displayName: String): String {
        if (parentDocumentId !in listOf("root", "success", "failure")) throw FileNotFoundException(parentDocumentId)
        val id = (if (parentDocumentId == "failure") "fail-" else "") + UUID.randomUUID()
        check(file(id).createNewFile())
        preferences.edit().putString("$id.name", displayName).putString("$id.mime", mimeType)
            .putString("$id.parent", parentDocumentId).commit()
        return id
    }
    override fun deleteDocument(documentId: String) {
        if (!file(documentId).delete()) throw FileNotFoundException(documentId)
        requireNotNull(context).getSharedPreferences("reports", 0).edit()
            .putString("$documentId.status", "deleted").commit()
        preferences.edit().remove("$documentId.name").remove("$documentId.mime").remove("$documentId.parent").commit()
    }
    override fun openDocument(documentId: String, mode: String, signal: CancellationSignal?): ParcelFileDescriptor {
        if (!file(documentId).isFile) throw FileNotFoundException(documentId)
        if (documentId.startsWith("fail-") && mode.contains('w')) {
            val pipe = ParcelFileDescriptor.createReliablePipe()
            Thread({
                try { ParcelFileDescriptor.AutoCloseInputStream(pipe[0]).use { it.read(ByteArray(32)) } }
                catch (_: java.io.IOException) { }
            }, "memedock-test-failing-sink").start()
            return pipe[1]
        }
        return ParcelFileDescriptor.open(file(documentId), ParcelFileDescriptor.parseMode(mode))
    }
}
