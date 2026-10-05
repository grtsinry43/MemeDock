package com.grtsinry43.memedock.sharetest

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import android.os.Bundle
import androidx.core.content.FileProvider
import java.io.File

class SenderActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Initialize the provider before looking up its generated files.
        contentResolver.getType(android.net.Uri.parse("content://$packageName.fixtures/fixtures/MemeDock-shared.png"))
        val first = FileProvider.getUriForFile(this, "$packageName.fixtures", File(filesDir, "fixtures/MemeDock-shared.png"))
        val second = FileProvider.getUriForFile(this, "$packageName.fixtures", File(filesDir, "fixtures/MemeDock-shared-second.png"))
        if (intent.getBooleanExtra("revoke", false)) {
            revokeUriPermission(first, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            revokeUriPermission(second, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            intent.getStringExtra("reportKey")?.let { key -> getSharedPreferences("reports", 0).edit().putString("$key.status", "revoked").commit() }
            finish(); return
        }
        val multiple = intent.getBooleanExtra("multiple", false)
        val send = Intent(if (multiple) Intent.ACTION_SEND_MULTIPLE else Intent.ACTION_SEND).apply {
            setClassName("com.grtsinry43.memedock", "com.grtsinry43.memedock.MainActivity")
            type = "image/png"
            if (multiple) putParcelableArrayListExtra(Intent.EXTRA_STREAM, arrayListOf(first, second))
            else putExtra(Intent.EXTRA_STREAM, first)
            clipData = ClipData.newRawUri("test images", first).apply { if (multiple) addItem(ClipData.Item(second)) }
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        startActivity(send)
        finish()
    }
}
