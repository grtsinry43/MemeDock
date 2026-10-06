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

class ManagementDeviceTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun collectionMemberOrderAndTagManagementUseRealNativeChanges() = runBlocking {
        val container = (compose.activity.application as MemeDockApplication).container
        val repository = container.library
        val suffix = System.nanoTime().toString()
        val collection = repository.createCollection("顺序$suffix")
        val ids = mutableListOf<String>()
        compose.runOnIdle { container.imports.hide() }
        try {
            repeat(2) { index ->
                val input = repository.createInput()
                val bitmap = Bitmap.createBitmap(18, 18, Bitmap.Config.ARGB_8888).apply { eraseColor((suffix.toLong() + index).toInt() or 0xff000000.toInt()) }
                try {
                    File(input.path).outputStream().use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
                    ids += repository.importInput(input, "顺序$index-$suffix.png", collection.id) { false }.id
                } finally { bitmap.recycle(); repository.discardInput(input); input.close() }
            }
            compose.onNodeWithTag("tab:Collections").performClick()
            waitText(collection.name)
            compose.onNodeWithText(collection.name).performClick()
            compose.onNodeWithText("调整顺序").performClick()
            compose.waitUntil(10_000) { compose.onAllNodesWithTag("order-select:" + ids[1]).fetchSemanticsNodes().isNotEmpty() }
            compose.onNodeWithTag("order-select:" + ids[1]).performClick()
            compose.onNodeWithText("移到最前").performClick()
            compose.waitUntil(10_000) { runBlocking { repository.page("", collectionId = collection.id).let {
                try { it.items.firstOrNull()?.id == ids[1] } finally { it.next?.close() }
            } } }
            androidx.test.espresso.Espresso.pressBack()
            compose.onNodeWithContentDescription("返回合集").performClick()
            compose.onNodeWithTag("tab:Settings").performClick()
            compose.onNodeWithText("管理标签").performScrollTo().performClick()
            compose.onNodeWithText("新建标签").performClick()
            val tagName = "排序标签$suffix"
            compose.onNode(hasSetTextAction()).performTextReplacement(tagName)
            compose.onNodeWithText("保存").performClick()
            waitText(tagName)
            val tag = repository.tags().single { it.name == tagName }
            compose.onNode(hasContentDescription("整理") and hasAnyAncestor(hasText(tagName))).performClick()
            compose.onNodeWithText("重命名").performClick()
            compose.onNode(hasSetTextAction()).performTextReplacement("改名$suffix")
            compose.onNodeWithText("保存").performClick()
            waitText("改名$suffix")
            assertEquals(tag.id, repository.tags().single { it.name == "改名$suffix" }.id)
        } finally {
            ids.forEach { repository.detail(it).let { value -> if (!value.deleted) repository.deleteSticker(value) } }
            repository.collections(false).firstOrNull { it.id == collection.id }?.let { repository.deleteCollection(it) }
            repository.tags().filter { it.name.endsWith(suffix) }.forEach { repository.deleteTag(it) }
        }
        Unit
    }

    @Test fun realEditingOrganizingDeletionAndRecoveryRefreshEveryPage() = runBlocking {
        val container = (compose.activity.application as MemeDockApplication).container
        val repository = container.library
        val unique = System.nanoTime().toString()
        val title = "整理猫$unique"
        val collectionName = "合集$unique"
        val tagName = "标签$unique"
        compose.runOnIdle { container.imports.hide() }
        val input = repository.createInput()
        val bitmap = Bitmap.createBitmap(24, 24, Bitmap.Config.ARGB_8888).apply { eraseColor(unique.toLong().toInt() or 0xff000000.toInt()) }
        val id = try {
            File(input.path).outputStream().use { assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
            repository.importInput(input, "Management-$unique.png") { false }.id
        } finally { bitmap.recycle(); repository.discardInput(input); input.close() }
        try {
            compose.onNodeWithTag("tab:Collections").performClick()
            compose.onNodeWithText("新建合集").performClick()
            compose.onNode(hasSetTextAction()).performTextReplacement(collectionName)
            compose.onNodeWithText("保存").performClick()
            waitText(collectionName)
            compose.onNodeWithTag("tab:Stickers").performClick()
            compose.waitUntil(10_000) { compose.onAllNodesWithTag("sticker:$id").fetchSemanticsNodes().isNotEmpty() }
            compose.onNodeWithTag("sticker:$id").performClick()
            waitText("编辑图片")
            compose.onNodeWithText("编辑图片").performScrollTo().performClick()
            compose.onAllNodes(hasSetTextAction())[0].performTextReplacement(title)
            compose.onAllNodes(hasSetTextAction())[1].performTextReplacement("明天再说")
            compose.onNodeWithText("保存").performScrollTo().performClick()
            compose.waitUntil(10_000) { compose.onAllNodesWithTag("detail-name").fetchSemanticsNodes().firstOrNull()?.config
                ?.getOrElse(androidx.compose.ui.semantics.SemanticsProperties.Text) { emptyList() }?.any { it.text == title } == true }
            compose.onNodeWithText("收藏", substring = false).performScrollTo().performClick()
            waitText("取消收藏")
            compose.onNodeWithText("整理", substring = false).performScrollTo().performClick()
            waitText(collectionName)
            compose.onNodeWithText(collectionName).performClick()
            compose.onNode(hasSetTextAction()).performTextReplacement(tagName)
            compose.onNode(hasText("新建标签", substring = false) and SemanticsMatcher.expectValue(
                androidx.compose.ui.semantics.SemanticsProperties.Role, androidx.compose.ui.semantics.Role.Button)).performClick()
            waitText(tagName)
            compose.onNodeWithText("保存").performClick()
            compose.waitUntil(10_000) { runBlocking { repository.detail(id).tags.any { it.name == tagName } } }
            val detail = repository.detail(id)
            assertEquals(title, detail.title)
            assertTrue(detail.starred)
            assertEquals(collectionName, detail.collections.single().name)
            compose.onNodeWithText("移入回收站").performScrollTo().performClick()
            compose.onNodeWithTag("confirm-delete-sticker").performClick()
            compose.waitUntil(10_000) { runBlocking { repository.detail(id).deleted } }
            compose.onNodeWithTag("tab:Settings").performClick()
            compose.onNodeWithText("回收站").performScrollTo().performClick()
            waitText(title)
            compose.onNodeWithText(title).performClick()
            compose.waitUntil(10_000) { compose.onAllNodesWithTag("restore-sticker").fetchSemanticsNodes().isNotEmpty() }
            compose.onNodeWithTag("restore-sticker").performScrollTo().performClick()
            waitText("原来的整理")
            compose.onNodeWithTag("confirm-restore-sticker").performClick()
            compose.waitUntil(10_000) { runBlocking { !repository.detail(id).deleted } }
            val restored = repository.detail(id)
            assertEquals(1L, restored.generation)
            assertTrue(restored.collections.isEmpty())
            assertTrue(restored.tags.isEmpty())
            assertTrue(restored.starred)
        } finally {
            val detail = repository.detail(id)
            if (!detail.deleted) repository.deleteSticker(detail)
            repository.collections(false).filter { it.name == collectionName }.forEach { repository.deleteCollection(it) }
            repository.tags().filter { it.name == tagName }.forEach { repository.deleteTag(it) }
        }
        Unit
    }

    private fun waitText(text: String) {
        compose.waitUntil(10_000) { compose.onAllNodesWithText(text, substring = false).fetchSemanticsNodes().isNotEmpty() }
    }
}
