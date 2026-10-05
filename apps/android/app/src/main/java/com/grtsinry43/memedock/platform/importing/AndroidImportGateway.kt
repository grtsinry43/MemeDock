package com.grtsinry43.memedock.platform.importing

import android.content.ContentResolver
import android.net.Uri
import android.os.CancellationSignal
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.FileOutputStream
import java.io.InputStream
import java.util.concurrent.atomic.AtomicReference
import com.grtsinry43.memedock.data.library.LibraryFailure

class AndroidImportGateway(private val resolver: ContentResolver, private val maxBytes: Long = 32L * 1024 * 1024) : ImportGateway {
    private data class Read(val signal: CancellationSignal, val input: AtomicReference<InputStream?> = AtomicReference())
    private val active = AtomicReference<Read?>()
    override suspend fun describe(uri: String): ImportCandidate = withContext(Dispatchers.IO) {
        var name = "未命名图片"
        var size: Long? = null
        resolver.query(Uri.parse(uri), arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) {
                val nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                val sizeIndex = cursor.getColumnIndex(OpenableColumns.SIZE)
                if (nameIndex >= 0 && !cursor.isNull(nameIndex)) name = cursor.getString(nameIndex)
                if (sizeIndex >= 0 && !cursor.isNull(sizeIndex)) size = cursor.getLong(sizeIndex).takeIf { it >= 0 }
            }
        }
        ImportCandidate(uri, name, size)
    }
    override suspend fun copy(candidate: ImportCandidate, destination: String, cancelled: () -> Boolean, progress: (Long) -> Unit) = withContext(Dispatchers.IO) {
        val read = Read(CancellationSignal())
        check(active.compareAndSet(null, read)) { "Another URI read is active" }
        try {
            if (cancelled()) throw LibraryFailure("CANCELLED")
            resolver.openAssetFileDescriptor(Uri.parse(candidate.uri), "r", read.signal)?.use { descriptor ->
                descriptor.createInputStream().use { input ->
                    read.input.set(input)
                    FileOutputStream(destination).use { output ->
                        val buffer = ByteArray(64 * 1024)
                        var bytes = 0L
                        var lastReported = 0L
                        while (true) {
                            if (cancelled()) throw LibraryFailure("CANCELLED")
                            val count = input.read(buffer, 0, minOf(buffer.size.toLong(), maxBytes - bytes + 1).toInt())
                            if (count == -1) break
                            bytes += count
                            if (bytes > maxBytes) throw LibraryFailure("RESOURCE_LIMIT")
                            output.write(buffer, 0, count)
                            if (bytes - lastReported >= 256 * 1024) { progress(bytes); lastReported = bytes }
                        }
                        output.fd.sync()
                        progress(bytes)
                    }
                }
            } ?: throw LibraryFailure("IO")
        } finally { active.compareAndSet(read, null) }
    }
    override suspend fun cancelActiveRead() = withContext(Dispatchers.IO) {
        active.get()?.let { read ->
            read.signal.cancel()
            read.input.get()?.close()
        }
        Unit
    }
}
