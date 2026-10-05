package com.grtsinry43.memedock.ui.components

import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class MemeDockMessageTest {
    @Test fun queueCancelAndActionPreserveTheResultOfEachCaller() = runTest {
        val state = SnackbarHostState()
        val firstMessage = MemeDockMessage("原图已保存", MemeDockMessageType.Success)
        val first = async { state.showMemeDockMessage(firstMessage) }
        runCurrent()
        assertEquals(firstMessage, state.currentSnackbarData?.visuals)
        val cancelled = async { state.showMemeDockMessage(MemeDockMessage("取消这条", MemeDockMessageType.Warning)) }
        val lastMessage = MemeDockMessage("导出失败", MemeDockMessageType.Error, actionLabel = "重试")
        val last = async { state.showMemeDockMessage(lastMessage) }
        runCurrent()
        cancelled.cancel()
        runCurrent()
        state.currentSnackbarData?.dismiss()
        runCurrent()
        assertEquals(SnackbarResult.Dismissed, first.await())
        assertEquals(lastMessage, state.currentSnackbarData?.visuals)
        state.currentSnackbarData?.performAction()
        runCurrent()
        assertEquals(SnackbarResult.ActionPerformed, last.await())
        assertNull(state.currentSnackbarData)
    }

    @Test fun cancellingTheVisibleMessageReleasesTheHostForTheNextCaller() = runTest {
        val state = SnackbarHostState()
        val visible = async { state.showMemeDockMessage(MemeDockMessage("第一条", MemeDockMessageType.Warning)) }
        runCurrent()
        val next = async { state.showMemeDockMessage(MemeDockMessage("第二条", MemeDockMessageType.Success)) }
        runCurrent()
        visible.cancel()
        runCurrent()
        assertEquals("第二条", state.currentSnackbarData?.visuals?.message)
        state.currentSnackbarData?.dismiss()
        runCurrent()
        assertEquals(SnackbarResult.Dismissed, next.await())
        assertNull(state.currentSnackbarData)
    }

    @Test fun messagesRequireTextAndActionsAreNotTimedOutByDefault() {
        assertEquals(SnackbarDuration.Short, MemeDockMessage("完成", MemeDockMessageType.Success).duration)
        assertEquals(SnackbarDuration.Indefinite,
            MemeDockMessage("失败", MemeDockMessageType.Error, actionLabel = "重试").duration)
        assertThrows(IllegalArgumentException::class.java) { MemeDockMessage(" ", MemeDockMessageType.Success) }
        assertThrows(IllegalArgumentException::class.java) {
            MemeDockMessage("失败", MemeDockMessageType.Error, actionLabel = " ")
        }
    }
}
