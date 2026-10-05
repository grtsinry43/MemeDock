package com.grtsinry43.memedock

import android.os.Bundle
import android.content.Intent
import com.grtsinry43.memedock.platform.importing.ShareIntentParser
import com.grtsinry43.memedock.data.library.LibraryFailure
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.grtsinry43.memedock.ui.MemeDockApp
import com.grtsinry43.memedock.app.MemeDockApplication

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val container = (application as MemeDockApplication).container
        if (savedInstanceState == null) receive(intent)
        setContent { MemeDockApp(container) }
    }
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        receive(intent)
    }
    private fun receive(intent: Intent) {
        val imports = (application as MemeDockApplication).container.imports
        try {
            val uris = ShareIntentParser.parse(intent)
            if (uris.isNotEmpty()) imports.prepare(uris, eager = true)
        } catch (error: LibraryFailure) { imports.selectionFailed(error.reason) }
    }
}
