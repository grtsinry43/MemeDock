package com.grtsinry43.memedock.feature

import android.graphics.Bitmap
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import com.grtsinry43.memedock.MainActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File

class LibraryDetailTransitionTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun imageTransitionReturnsToTheSameSearchAndScrollAndCanBeInterrupted() = runBlocking {
        val container = (compose.activity.application as MemeDockApplication).container
        compose.runOnIdle { container.imports.hide() }
        val ids = (0 until 18).map { index ->
            val input = container.library.createInput()
            val bitmap = Bitmap.createBitmap(96, 64, Bitmap.Config.ARGB_8888).apply { eraseColor(0xff8060a0.toInt() + index * 8192) }
            try {
                File(input.path).outputStream().use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
                container.library.importInput(input, "UX-motion-${index.toString().padStart(2, '0')}.png") { false }.id
            } finally { bitmap.recycle(); container.library.discardInput(input); input.close() }
        }
        compose.onNodeWithTag("tab:Search").performClick()
        compose.onNodeWithTag("library-search").performTextReplacement("UX-motion")
        compose.waitUntil(10_000) { ids.any { compose.onAllNodesWithTag("sticker:$it").fetchSemanticsNodes().isNotEmpty() } }
        androidx.test.espresso.Espresso.closeSoftKeyboard()
        val tile = "sticker:${ids[15]}"
        compose.onNodeWithTag("library-grid").performScrollToNode(hasTestTag(tile))
        val original = compose.onNodeWithTag(tile).fetchSemanticsNode().boundsInRoot
        compose.mainClock.autoAdvance = false
        try {
            compose.onNodeWithTag(tile).performClick()
            compose.mainClock.advanceTimeBy(96)
            compose.onNodeWithTag("detail-preview").assertExists()
            compose.mainClock.advanceTimeBy(800)
            compose.waitUntil(10_000) { compose.onAllNodesWithTag("detail-name").fetchSemanticsNodes().isNotEmpty() }
            compose.onNodeWithText("分享原图").assertIsDisplayed()
            compose.onNodeWithContentDescription("返回图片库").performClick()
            compose.mainClock.advanceTimeBy(800)
            compose.onNodeWithTag("library-search").assertTextContains("UX-motion")
            val restored = compose.onNodeWithTag(tile).fetchSemanticsNode().boundsInRoot
            assertEquals(original.top, restored.top, 2f)
            // Reverse the transition before it finishes; both destinations must
            // remain valid, and the library must still own its saved grid state.
            compose.onNodeWithTag(tile).performClick()
            compose.mainClock.advanceTimeBy(48)
            compose.onNodeWithContentDescription("返回图片库").performClick()
            compose.mainClock.advanceTimeBy(800)
            compose.onNodeWithTag(tile).assertIsDisplayed()
            compose.onNodeWithTag("library-search").assertTextContains("UX-motion")
        } finally { compose.mainClock.autoAdvance = true }
        Unit
    }
}
