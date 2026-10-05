package com.grtsinry43.memedock.feature

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import com.grtsinry43.memedock.data.library.LibraryCollection
import com.grtsinry43.memedock.feature.collections.*
import com.grtsinry43.memedock.ui.theme.MemeDockTheme
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class CollectionsScreenTest {
    @get:Rule val compose = createComposeRule()
    @Test fun collectionSelectionPassesTheActualIdentity() {
        val collection = LibraryCollection("collection-identity", "猫猫")
        var selected: LibraryCollection? = null
        compose.setContent { MemeDockTheme { CollectionsScreen(CollectionsState(listOf(collection), false), {}, { selected = it }) } }
        compose.onNodeWithText("猫猫").performClick()
        assertEquals(collection, selected)
    }
    @Test fun failedCollectionReadOffersRetry() {
        var retried = false
        compose.setContent { MemeDockTheme { CollectionsScreen(CollectionsState(loading = false, error = "BUSY"), { retried = true }, {}) } }
        compose.onNodeWithText("重试").performClick()
        assertTrue(retried)
    }
}
