package com.grtsinry43.memedock.ui.components

import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.TextField
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.AccessibilityManager
import androidx.compose.ui.platform.LocalAccessibilityManager
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.unit.Density
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.grtsinry43.memedock.ui.theme.MemeDockTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.atomic.AtomicBoolean

@RunWith(AndroidJUnit4::class)
class MemeDockMessageHostTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    private val host = SnackbarHostState()
    private lateinit var scope: CoroutineScope

    private fun install(dark: Boolean = false, fontScale: Float = 1f) {
        compose.setContent {
            val density = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(density.density, fontScale)) {
                MemeDockTheme(darkTheme = dark) {
                    scope = rememberCoroutineScope()
                    Scaffold(snackbarHost = { MemeDockMessageHost(host) }) { padding ->
                        Column(Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding)) {}
                    }
                }
            }
        }
    }

    @Test fun statusIsAnnouncedAndActionReturnsItsResult() {
        install()
        var result: SnackbarResult? = null
        compose.runOnIdle {
            scope.launch { result = host.showMemeDockMessage(
                MemeDockMessage("导出失败，请重试", MemeDockMessageType.Error, actionLabel = "重试")) }
        }
        compose.onNodeWithText("导出失败，请重试").assertIsDisplayed()
        compose.onNodeWithContentDescription("错误").assertExists()
        compose.onAllNodes(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Polite))
            .assertCountEquals(1)
        compose.onNodeWithText("重试").performClick()
        compose.runOnIdle { assertEquals(SnackbarResult.ActionPerformed, result) }
    }

    @Test fun dismissThenCancelRemovesBothVisibleAndQueuedMessages() {
        install(dark = true)
        var dismissed: SnackbarResult? = null
        var second: Job? = null
        compose.runOnIdle {
            scope.launch { dismissed = host.showMemeDockMessage(
                MemeDockMessage("保存成功", MemeDockMessageType.Success, duration = SnackbarDuration.Indefinite)) }
            second = scope.launch { host.showMemeDockMessage(
                MemeDockMessage("空间不足", MemeDockMessageType.Warning, duration = SnackbarDuration.Indefinite)) }
        }
        compose.onNodeWithText("保存成功").assertIsDisplayed()
        compose.onNodeWithContentDescription("关闭提示").performClick()
        compose.onNodeWithText("空间不足").assertIsDisplayed()
        compose.runOnIdle {
            assertEquals(SnackbarResult.Dismissed, dismissed)
            second?.cancel()
        }
        compose.waitForIdle()
        compose.runOnIdle { assertNull(host.currentSnackbarData) }
    }

    @Test fun closeButtonParticipatesInTheAccessibilityTimeout() {
        val controls = AtomicBoolean(false)
        val manager = object : AccessibilityManager {
            override fun calculateRecommendedTimeoutMillis(originalTimeoutMillis: Long,
                containsIcons: Boolean, containsText: Boolean, containsControls: Boolean): Long {
                controls.set(containsIcons && containsText && containsControls)
                return 60_000L
            }
        }
        compose.setContent {
            CompositionLocalProvider(LocalAccessibilityManager provides manager) {
                MemeDockTheme {
                    scope = rememberCoroutineScope()
                    MemeDockMessageHost(host)
                }
            }
        }
        compose.runOnIdle { scope.launch { host.showMemeDockMessage(MemeDockMessage("完成", MemeDockMessageType.Success)) } }
        compose.onNodeWithText("完成").assertIsDisplayed()
        compose.runOnIdle { assertTrue("Dismiss control must receive the user's timeout", controls.get()) }
        compose.onNodeWithContentDescription("关闭提示").performClick()
    }

    @Test fun longChineseMessageAndActionsFitAtDoubleFontScale() {
        install(fontScale = 2f)
        val text = "无法完成导出，请检查设备剩余空间与目标文件夹的访问权限，然后重新尝试。"
        compose.runOnIdle { scope.launch { host.showMemeDockMessage(
            MemeDockMessage(text, MemeDockMessageType.Warning, actionLabel = "重新尝试")) } }
        compose.onNodeWithText(text).assertIsDisplayed()
        compose.onNodeWithText("重新尝试").assertIsDisplayed()
        compose.onNodeWithContentDescription("关闭提示").assertIsDisplayed()
        val textBounds = compose.onNodeWithText(text).fetchSemanticsNode().boundsInRoot
        val actionBounds = compose.onNodeWithText("重新尝试").fetchSemanticsNode().boundsInRoot
        assertTrue("Action must remain below the multiline text", textBounds.bottom <= actionBounds.top)
        compose.onNodeWithText("重新尝试").performClick()
    }

    @Suppress("DEPRECATION") // adjustResize matches the manifest and supports IME insets on API 28.
    @Test fun keyboardDoesNotCoverTheMessage() {
        var imeBottom = 0
        val input = mutableStateOf("")
        compose.runOnUiThread {
            // Match MainActivity rather than the test manifest's default adjustPan window.
            compose.activity.enableEdgeToEdge()
            compose.activity.window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
        }
        compose.setContent {
            MemeDockTheme {
                scope = rememberCoroutineScope()
                val bottom = WindowInsets.ime.getBottom(LocalDensity.current)
                SideEffect { imeBottom = bottom }
                Scaffold(snackbarHost = { MemeDockMessageHost(host) }) { padding ->
                    Column(Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding)) {
                        TextField(input.value, { input.value = it }, modifier = Modifier.testTag("input"))
                    }
                }
            }
        }
        compose.onNodeWithTag("input").performClick()
        compose.waitUntil(10_000) { imeBottom > 0 }
        compose.runOnIdle { scope.launch { host.showMemeDockMessage(
            MemeDockMessage("键盘上方的提示", MemeDockMessageType.Success, duration = SnackbarDuration.Indefinite)) } }
        val message = compose.onNodeWithText("键盘上方的提示").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        val root = compose.onRoot().fetchSemanticsNode().boundsInRoot
        assertTrue("Message must remain above the IME", message.bottom <= root.bottom - imeBottom)
        compose.onNodeWithContentDescription("关闭提示").performClick()
    }
}
