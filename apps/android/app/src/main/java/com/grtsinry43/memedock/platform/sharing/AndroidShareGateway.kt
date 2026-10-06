package com.grtsinry43.memedock.platform.sharing

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import android.content.Context
import androidx.core.content.FileProvider
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.ShareArtifact
import java.io.File

class AndroidShareGateway : ShareGateway {
    override fun share(activity: Activity, artifact: ShareArtifact) {
        check(!activity.isFinishing && !activity.isDestroyed) { "Activity unavailable" }
        activity.startActivity(Intent.createChooser(intentFor(activity, artifact), activity.getString(R.string.share_sticker)))
    }
    fun intentFor(context: Context, artifact: ShareArtifact): Intent {
        val uri = uriFor(context, artifact)
        return Intent(Intent.ACTION_SEND).apply {
            type = artifact.mime
            putExtra(Intent.EXTRA_STREAM, uri)
            clipData = ClipData.newUri(context.contentResolver, artifact.fileName, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
    }
    fun uriFor(context: Context, artifact: ShareArtifact): android.net.Uri {
        val file = File(artifact.path).canonicalFile
        val root = File(context.filesDir, "share").canonicalFile
        require(file.parentFile == root && file.isFile) { "Invalid sharing artifact" }
        return FileProvider.getUriForFile(context, "${context.packageName}.files", file, artifact.fileName)
    }
}
