package com.grtsinry43.memedock.feature

import android.content.ClipboardManager
import android.content.ClipData
import android.content.Context
import android.graphics.BitmapFactory
import android.view.KeyEvent
import androidx.activity.compose.setContent
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import com.grtsinry43.memedock.MainActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.feature.detail.DetailRoute
import com.grtsinry43.memedock.ui.theme.MemeDockTheme
import kotlinx.coroutines.*
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File

class ExportDeviceTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()
    @Test fun presetSelectionRequiresAnimationConsentAndSystemSaveCancellationReturnsToDetail() = runBlocking {
        withTimeout(40_000) {
            val instrumentation = InstrumentationRegistry.getInstrumentation()
            val context = instrumentation.targetContext
            val container = (context.applicationContext as MemeDockApplication).container
            val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            compose.runOnIdle { clipboard.setPrimaryClip(ClipData.newPlainText("test", "")) }
            val input = container.library.createInput()
            File(input.path).writeBytes(android.util.Base64.decode(
                "R0lGODlhDAAIAPAAAP8AAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQAMgAAACwAAAAADAAIAAACCISPqcvtD2MqACH5BAAyAAAALAAAAAAMAAgAgAAA/wAAAAIIhI+py+0PYyoAOw==", android.util.Base64.DEFAULT))
            val imported = try { container.library.importInput(input, "export-animation.gif") { false } }
                finally { container.library.discardInput(input); input.close() }
            try {
                container.exportPreferences.select(ExportChoice.Original)
                compose.activityRule.scenario.onActivity { activity ->
                    activity.setContent { MemeDockTheme { DetailRoute(imported.id, container, null, {}) } }
                }
                compose.waitUntil(10_000) { compose.onAllNodesWithTag("choose-export-preset").fetchSemanticsNodes().isNotEmpty() }
                compose.onNodeWithTag("choose-export-preset").performClick()
                compose.onNodeWithTag("export-preset-CompatiblePng").performClick()
                compose.waitUntil(5_000) { compose.onAllNodesWithTag("export-preset-CompatiblePng").fetchSemanticsNodes().isEmpty() }
                compose.onNodeWithTag("copy-image").performClick()
                compose.onNodeWithText("只使用第一帧？").assertIsDisplayed()
                compose.onNodeWithText("取消").performClick()
                compose.runOnIdle { assertNull(clipboard.primaryClip?.getItemAt(0)?.uri) }
                compose.onNodeWithTag("copy-image").performClick()
                compose.onNodeWithTag("confirm-first-frame").performClick()
                compose.waitUntil(10_000) { compose.onAllNodesWithTag("copy-image").fetchSemanticsNodes().isNotEmpty() }
                compose.runOnIdle {
                    assertEquals("image/png", clipboard.primaryClipDescription?.getMimeType(0))
                    assertNotNull(clipboard.primaryClip?.getItemAt(0)?.uri)
                }
                val copiedUri = withContext(Dispatchers.Main) { requireNotNull(clipboard.primaryClip?.getItemAt(0)?.uri) }
                val copied = withContext(Dispatchers.IO) {
                    context.contentResolver.openInputStream(copiedUri)?.use { BitmapFactory.decodeStream(it) }
                }
                requireNotNull(copied).let { bitmap ->
                    try { assertEquals(12, bitmap.width); assertEquals(8, bitmap.height) }
                    finally { bitmap.recycle() }
                }
                compose.onNodeWithTag("save-image").performClick()
                compose.onNodeWithTag("confirm-first-frame").performClick()
                while (!container.saves.state.value.selecting) delay(100)
                delay(500)
                compose.activityRule.scenario.recreate()
                assertTrue(container.saves.state.value.selecting)
                instrumentation.sendKeyDownUpSync(KeyEvent.KEYCODE_BACK)
                while (container.saves.state.value.artifact != null) delay(100)
                compose.onNodeWithTag("save-image").assertIsDisplayed()
                assertFalse(container.saves.state.value.saved)
            } finally { container.exportPreferences.select(ExportChoice.Original) }
        }
    }
}
