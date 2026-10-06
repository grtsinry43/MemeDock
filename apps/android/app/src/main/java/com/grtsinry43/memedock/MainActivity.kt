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
import androidx.activity.result.contract.ActivityResultContract
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.Lifecycle
import kotlinx.coroutines.launch
import com.grtsinry43.memedock.data.library.ShareArtifact
import android.content.Context
import android.net.Uri

class MainActivity : ComponentActivity() {
    private val backupSaveLauncher = registerForActivityResult(ActivityResultContracts.CreateDocument("application/zip")) {
        (application as MemeDockApplication).container.backups.savedTo(it)
    }
    private val backupOpenLauncher = registerForActivityResult(ActivityResultContracts.OpenDocument()) {
        (application as MemeDockApplication).container.backups.opened(it)
    }
    private val saveLauncher = registerForActivityResult(object : ActivityResultContract<ShareArtifact, Uri?>() {
        override fun createIntent(context: Context, input: ShareArtifact): Intent =
            ActivityResultContracts.CreateDocument(input.mime).createIntent(context, input.fileName)
        override fun parseResult(resultCode: Int, intent: Intent?): Uri? =
            if (resultCode == RESULT_OK) intent?.data else null
    }) { destination -> (application as MemeDockApplication).container.saves.selected(destination) }
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val container = (application as MemeDockApplication).container
        if (savedInstanceState == null) receive(intent)
        setContent { MemeDockApp(container) }
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                launch {
                    container.backups.state.collect {
                        val request = container.backups.claimPicker() ?: return@collect
                        try {
                            when (request) {
                                is com.grtsinry43.memedock.feature.backup.BackupPicker.Save -> backupSaveLauncher.launch(request.fileName)
                                com.grtsinry43.memedock.feature.backup.BackupPicker.Open -> backupOpenLauncher.launch(arrayOf("*/*"))
                            }
                        } catch (error: Exception) { container.backups.pickerFailed(error) }
                    }
                }
                container.saves.state.collect {
                    val artifact = container.saves.selecting() ?: return@collect
                    try { saveLauncher.launch(artifact) }
                    catch (error: Exception) { container.saves.launchFailed(error) }
                }
            }
        }
    }
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) (application as MemeDockApplication).container.clipboard.foreground { hasWindowFocus() }
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
