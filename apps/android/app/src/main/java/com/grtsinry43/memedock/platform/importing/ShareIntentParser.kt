package com.grtsinry43.memedock.platform.importing

import android.content.Intent
import android.net.Uri
import androidx.core.content.IntentCompat
import com.grtsinry43.memedock.data.library.LibraryFailure

object ShareIntentParser {
    fun parse(intent: Intent): List<String> {
        if (intent.action !in listOf(Intent.ACTION_SEND, Intent.ACTION_SEND_MULTIPLE)) return emptyList()
        if (intent.type?.startsWith("image/") != true) throw LibraryFailure("UNSUPPORTED_FORMAT")
        val uris = linkedSetOf<String>()
        fun add(uri: Uri?) {
            if (uri == null) return
            if (uri.scheme != "content") throw LibraryFailure("INVALID_INPUT")
            uris.add(uri.toString())
            if (uris.size > 200) throw LibraryFailure("BATCH_LIMIT")
        }
        try {
            if (intent.action == Intent.ACTION_SEND) add(IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java))
            else IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)?.forEach(::add)
            intent.clipData?.let { clip ->
                if (clip.itemCount > 200) throw LibraryFailure("BATCH_LIMIT")
                repeat(clip.itemCount) { add(clip.getItemAt(it).uri) }
            }
        } catch (error: android.os.BadParcelableException) { throw LibraryFailure("INVALID_INPUT", error) }
        catch (error: ClassCastException) { throw LibraryFailure("INVALID_INPUT", error) }
        if (uris.isEmpty()) throw LibraryFailure("INVALID_INPUT")
        return uris.toList()
    }
}
