package com.grtsinry43.memedock.feature

import android.graphics.Bitmap
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import com.grtsinry43.memedock.MainActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import com.grtsinry43.memedock.data.settings.ThemeMode
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File

class BottomNavigationTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun fourTabsKeepSearchSeparateAndGridReachesBothEdges() = runBlocking {
        val container = (compose.activity.application as MemeDockApplication).container
        compose.runOnIdle { container.imports.hide() }
        val slot = container.library.createInput()
        val bitmap = Bitmap.createBitmap(24, 24, Bitmap.Config.ARGB_8888).apply { eraseColor(0xff35afcd.toInt()) }
        val id = try {
            File(slot.path).outputStream().use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
            container.library.importInput(slot, "Navigation-fixture.png") { false }.id
        } finally { bitmap.recycle(); container.library.discardInput(slot); slot.close() }
        compose.waitUntil(10_000) { compose.onAllNodesWithTag("sticker:$id").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithTag("library-search").assertDoesNotExist()
        val root = compose.onRoot().fetchSemanticsNode().boundsInRoot
        val grid = compose.onNodeWithTag("library-grid").fetchSemanticsNode().boundsInRoot
        assertEquals(root.left, grid.left, 1f)
        assertEquals(root.right, grid.right, 1f)
        compose.onNodeWithTag("tab:Search").performClick()
        compose.onNodeWithTag("library-search").performTextReplacement("Navigation-fixture")
        compose.onNodeWithTag("tab:Collections").performClick()
        compose.waitUntil(10_000) { compose.onAllNodesWithText("新建合集").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithTag("tab:Settings").performClick()
        compose.onNodeWithText("跟随系统").assertIsDisplayed()
        compose.waitUntil(10_000) { compose.onAllNodesWithText("已保存的原图").fetchSemanticsNodes().isNotEmpty() }
        try {
            compose.onNodeWithText("深色").performClick()
            compose.waitUntil(10_000) { compose.onAllNodesWithText("深色").fetchSemanticsNodes().singleOrNull()?.config?.getOrElse(androidx.compose.ui.semantics.SemanticsProperties.Selected) { false } == true }
            compose.onNodeWithTag("tab:Search").performClick()
            compose.onNodeWithTag("library-search").assertTextContains("Navigation-fixture")
            compose.onNodeWithTag("tab:Stickers").performClick()
            compose.onNodeWithTag("library-search").assertDoesNotExist()
        } finally { container.appearance.select(ThemeMode.System) }
        Unit
    }
}
