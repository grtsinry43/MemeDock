package com.grtsinry43.memedock.platform.clipboard

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import com.grtsinry43.memedock.data.library.ShareArtifact
import com.grtsinry43.memedock.platform.sharing.AndroidShareGateway

class AndroidClipboardGateway(context: Context) : ClipboardGateway {
    private val context = context.applicationContext
    private val manager = requireNotNull(context.getSystemService(ClipboardManager::class.java))
    override fun copy(artifact: ShareArtifact, reference: String) {
        val uri = AndroidShareGateway().uriFor(context, artifact).buildUpon()
            .appendQueryParameter("clipboard_reference", reference).build()
        // Explicit metadata avoids a provider query on the main thread.
        manager.setPrimaryClip(ClipData(ClipDescription(LABEL, arrayOf(artifact.mime)), ClipData.Item(uri)))
    }
    override fun observedReference(): String? {
        // The description is free to read; reading another app's clip content shows a system "pasted" notice.
        if (manager.primaryClipDescription?.label?.toString() != LABEL) return null
        val clip = manager.primaryClip ?: return null
        if (clip.itemCount != 1) return null
        val uri = clip.getItemAt(0).uri ?: return null
        if (uri.scheme != "content" || uri.authority != "${context.packageName}.files") return null
        return uri.getQueryParameter("clipboard_reference")?.takeIf {
            runCatching { java.util.UUID.fromString(it).toString() == it }.getOrDefault(false)
        }
    }
    private companion object { const val LABEL = "MemeDock" }
}
