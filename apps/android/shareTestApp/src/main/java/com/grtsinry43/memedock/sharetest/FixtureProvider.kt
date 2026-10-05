package com.grtsinry43.memedock.sharetest

import android.content.ContentProvider
import android.content.ContentValues
import android.database.MatrixCursor
import android.graphics.Bitmap
import android.net.Uri
import androidx.core.content.FileProvider
import java.io.File

class FixtureProvider : FileProvider(R.xml.fixture_paths) {
    override fun onCreate(): Boolean {
        val context = requireNotNull(context)
        val directory = File(context.filesDir, "fixtures").apply { mkdirs() }
        val bitmap = Bitmap.createBitmap(80, 40, Bitmap.Config.ARGB_8888).apply { eraseColor(0x804080C0.toInt()) }
        try {
            File(directory, "MemeDock-shared.png").outputStream().use { check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
            File(directory, "MemeDock-shared-second.png").outputStream().use { check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
        } finally { bitmap.recycle() }
        return super.onCreate()
    }
}

/** Read-only reports contain only generated test fixture results. */
class ReportProvider : ContentProvider() {
    override fun onCreate() = true
    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): MatrixCursor {
        val report = requireNotNull(context).getSharedPreferences("reports", 0)
        val key = uri.lastPathSegment ?: "default"
        return MatrixCursor(arrayOf("status", "bytes", "hash")).apply {
            addRow(arrayOf(report.getString("$key.status", "pending"), report.getLong("$key.bytes", 0), report.getString("$key.hash", "")))
        }
    }
    override fun getType(uri: Uri) = "vnd.android.cursor.item/vnd.memedock.sharetest.report"
    override fun insert(uri: Uri, values: ContentValues?): Uri = throw UnsupportedOperationException("Read-only reports")
    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?) = throw UnsupportedOperationException("Read-only reports")
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?) = throw UnsupportedOperationException("Read-only reports")
}
