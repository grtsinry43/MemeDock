package com.grtsinry43.memedock.feature

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import coil3.ImageLoader
import com.grtsinry43.memedock.feature.library.LibraryScreen
import com.grtsinry43.memedock.feature.library.LibraryUiState
import com.grtsinry43.memedock.ui.theme.MemeDockTheme
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class LibraryScreenTest {
    @get:Rule val compose = createComposeRule()
    @Test fun emptyLibraryOffersRealSelectionAndAccessibleSearch() {
        var selected = false
        var search = ""
        val loader = ImageLoader(InstrumentationRegistry.getInstrumentation().targetContext)
        try {
            compose.setContent {
                MemeDockTheme {
                    LibraryScreen(LibraryUiState(loading = false), loader, { search = it }, {}, {}, { _, _ -> }, { selected = true }, {}, false, {})
                }
            }
            compose.onNodeWithText("还没有图片，导入第一张吧").assertIsDisplayed()
            compose.onNodeWithText("导入图片").performClick()
            assertTrue(selected)
            compose.onNode(hasSetTextAction()).performTextInput("猫猫")
            assertEquals("猫猫", search)
        } finally { loader.shutdown() }
    }
}
