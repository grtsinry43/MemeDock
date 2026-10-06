package com.grtsinry43.memedock.sharetest

import android.app.Activity
import android.net.Uri
import android.os.Bundle
import android.content.Intent
import android.widget.TextView
import androidx.core.content.IntentCompat
import java.security.MessageDigest
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

class ReceiverActivity : Activity() {
    private val executor = Executors.newSingleThreadScheduledExecutor()
    private var started = false
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(TextView(this).apply { text = "MemeDock：等待延迟读取"; textSize = 22f })
        if (!intent.getBooleanExtra("clipboard", false)) {
            read(IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java))
        }
    }
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus && intent.getBooleanExtra("clipboard", false) && !started) {
            val clipboard = requireNotNull(getSystemService(android.content.ClipboardManager::class.java))
            read(clipboard.primaryClip?.getItemAt(0)?.uri)
        }
    }
    private fun read(uri: Uri?) {
        if (started) return
        started = true
        val key = intent.getStringExtra("reportKey") ?: "default"
        val preferences = getSharedPreferences("reports", 0)
        preferences.edit().putString("$key.status", "pending").commit()
        executor.schedule({
            try {
                val digest = MessageDigest.getInstance("SHA-256")
                var bytes = 0L
                contentResolver.openInputStream(requireNotNull(uri)).use { stream ->
                    val input = requireNotNull(stream)
                    val buffer = ByteArray(64 * 1024)
                    while (true) { val read = input.read(buffer); if (read < 0) break; bytes += read; digest.update(buffer, 0, read) }
                }
                val hash = digest.digest().joinToString("") { "%02x".format(it) }
                preferences.edit().putString("$key.status", "read").putLong("$key.bytes", bytes).putString("$key.hash", hash).commit()
                runOnUiThread { finish() }
            } catch (error: Exception) { preferences.edit().putString("$key.status", error.javaClass.simpleName).commit(); runOnUiThread { finish() } }
            finally { executor.shutdown() }
        }, intent.getLongExtra("delaySeconds", 3), TimeUnit.SECONDS)
    }
}
