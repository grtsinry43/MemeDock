package com.grtsinry43.memedock.platform.saving

import android.content.ContentResolver
import android.net.Uri
import android.provider.DocumentsContract
import com.grtsinry43.memedock.data.library.ShareArtifact
import kotlinx.coroutines.*
import java.io.File
import java.io.IOException

class AndroidSaveGateway(private val resolver: ContentResolver) : SaveGateway {
    override suspend fun save(destination: Uri, artifact: ShareArtifact, cancelled: () -> Boolean) = withContext(Dispatchers.IO) {
        try {
            if (cancelled()) throw CancellationException("Save cancelled")
            File(artifact.path).inputStream().use { source ->
                val output = resolver.openOutputStream(destination, "wt") ?: throw IOException("Destination unavailable")
                output.use {
                    val buffer = ByteArray(64 * 1024)
                    var copied = 0L
                    while (true) {
                        if (cancelled()) throw CancellationException("Save cancelled")
                        val count = source.read(buffer)
                        if (count < 0) break
                        copied += count
                        if (copied > artifact.byteSize) throw IOException("Output changed")
                        output.write(buffer, 0, count)
                    }
                    if (copied != artifact.byteSize) throw IOException("Output changed")
                    if (cancelled()) throw CancellationException("Save cancelled")
                    output.flush()
                }
            }
        } catch (error: Exception) {
            try { withContext(NonCancellable) { discardCreatedDocument(destination) } }
            catch (cleanup: Exception) { error.addSuppressed(cleanup) }
            throw error
        }
    }
    override suspend fun discardCreatedDocument(destination: Uri): Unit = withContext(Dispatchers.IO) {
        if (!DocumentsContract.deleteDocument(resolver, destination)) throw IOException("Partial document could not be removed")
    }
}
