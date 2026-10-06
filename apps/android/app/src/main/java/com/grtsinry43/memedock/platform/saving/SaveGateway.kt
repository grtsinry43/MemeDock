package com.grtsinry43.memedock.platform.saving

import android.net.Uri
import com.grtsinry43.memedock.data.library.ShareArtifact

interface SaveGateway {
    suspend fun save(destination: Uri, artifact: ShareArtifact, cancelled: () -> Boolean)
    suspend fun discardCreatedDocument(destination: Uri)
}
