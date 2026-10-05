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
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(TextView(this).apply { text = "MemeDock：等待延迟读取"; textSize = 22f })
        val uri = IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
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
        }, 3, TimeUnit.SECONDS)
    }
}
