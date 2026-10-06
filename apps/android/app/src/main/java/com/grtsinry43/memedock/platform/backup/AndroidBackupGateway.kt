package com.grtsinry43.memedock.platform.backup

import android.content.ContentResolver
import android.net.Uri
import android.provider.DocumentsContract
import kotlinx.coroutines.*
import java.io.File
import java.io.IOException

/** SAF grants are used immediately. No filesystem path is inferred from a URI. */
class AndroidBackupGateway(private val resolver: ContentResolver) {
    suspend fun read(uri: Uri, path: String, maximum: Long, cancelled: () -> Boolean) = withContext(Dispatchers.IO) {
        require(uri.scheme == "content")
        val source = resolver.openInputStream(uri) ?: throw IOException("Source unavailable")
        source.use { input -> File(path).outputStream().use { output ->
            stream(input, output, maximum, cancelled)
            output.fd.sync()
        } }
    }
    suspend fun save(uri: Uri, path: String, size: Long, cancelled: () -> Boolean) = withContext(Dispatchers.IO) {
        try {
            File(path).inputStream().use { source ->
                val destination = resolver.openOutputStream(uri, "wt") ?: throw IOException("Destination unavailable")
                destination.use { output ->
                    if (stream(source, output, size, cancelled) != size) throw IOException("Backup changed")
                    output.flush()
                }
            }
        } catch (failure: Exception) {
            try { discard(uri) } catch (cleanup: Exception) { failure.addSuppressed(cleanup) }
            throw failure
        }
    }
    suspend fun discard(uri: Uri): Unit = withContext(NonCancellable + Dispatchers.IO) {
        if (!DocumentsContract.deleteDocument(resolver, uri)) throw IOException("Partial document could not be removed")
    }
    private fun stream(input: java.io.InputStream, output: java.io.OutputStream, maximum: Long, cancelled: () -> Boolean): Long {
        val buffer = ByteArray(64 * 1024)
        var copied = 0L
        while (true) {
            if (cancelled()) throw CancellationException("Archive copy cancelled")
            val count = input.read(buffer)
            if (count < 0) break
            copied += count
            if (copied > maximum) throw com.grtsinry43.memedock.data.library.LibraryFailure("RESOURCE_LIMIT")
            output.write(buffer, 0, count)
        }
        if (cancelled()) throw CancellationException("Archive copy cancelled")
        return copied
    }
}
