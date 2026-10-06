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
        if (intent.getBooleanExtra("createDocument", false)) {
            val parent = android.provider.DocumentsContract.buildDocumentUri("$packageName.documents",
                if (intent.getBooleanExtra("failWrite", false)) "failure" else "success")
            val uri = requireNotNull(android.provider.DocumentsContract.createDocument(contentResolver, parent,
                "image/png", "MemeDock-test.png"))
            grantUriPermission("com.grtsinry43.memedock", uri,
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
            val key = requireNotNull(intent.getStringExtra("reportKey"))
            getSharedPreferences("reports", 0).edit().putString("$key.status", "created").putString("$key.hash", uri.toString()).commit()
            finish(); return
        }
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
