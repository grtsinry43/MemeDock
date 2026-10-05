package com.grtsinry43.memedock.feature

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import com.grtsinry43.memedock.feature.importing.*
import com.grtsinry43.memedock.platform.importing.ImportCandidate
import com.grtsinry43.memedock.ui.theme.MemeDockTheme
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class ImportSheetTest {
    @get:Rule val compose = createComposeRule()
    @Test fun confirmationBackgroundAndRetryHaveDistinctActions() {
        val item = ImportItemState(ImportCandidate("content://test/1", "猫猫.png", 202), ImportItemStatus.Staged)
        val state = mutableStateOf(ImportState(phase = ImportPhase.Review, items = listOf(item)))
        var starts = 0; var retries = 0; var stops = 0; var hides = 0; var discards = 0
        compose.setContent { MemeDockTheme { ImportSheetContent(state.value, { starts++ }, { retries++ }, { stops++ }, { hides++ }, { discards++ }) } }
        compose.onNodeWithText("准备好了").assertIsDisplayed()
        compose.onNodeWithText("确认导入").performClick()
        compose.onNodeWithText("暂不导入").performClick()
        assertEquals(1, starts); assertEquals(1, discards); assertEquals(0, hides)
        compose.runOnIdle { state.value = state.value.copy(phase = ImportPhase.Running) }
        compose.onNodeWithText("后台继续").performClick()
        compose.onNodeWithText("停止导入").performClick()
        assertEquals(1, hides); assertEquals(1, stops)
        compose.runOnIdle { state.value = state.value.copy(phase = ImportPhase.Finished, items = listOf(item.copy(status = ImportItemStatus.Failed, error = "IO"))) }
        compose.onNodeWithText("重试未导入的图片").performClick()
        compose.onNodeWithText("完成").performClick()
        assertEquals(1, retries); assertEquals(2, hides)
    }
}
