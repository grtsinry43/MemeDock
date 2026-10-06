package com.grtsinry43.memedock.feature

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.runtime.*
import androidx.activity.ComponentActivity
import com.grtsinry43.memedock.app.MemeDockApplication
import android.graphics.drawable.AnimatedImageDrawable
import coil3.size.ScaleDrawable
import androidx.lifecycle.Lifecycle
import coil3.EventListener
import coil3.asDrawable
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import java.util.concurrent.atomic.AtomicReference
import java.io.File
import androidx.test.platform.app.InstrumentationRegistry
import coil3.ImageLoader
import com.grtsinry43.memedock.data.library.StickerDetails
import com.grtsinry43.memedock.feature.detail.*
import com.grtsinry43.memedock.ui.theme.MemeDockTheme
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class DetailScreenTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    @Test fun actualGifStopsWhenPausedAndWhenPreviewLeavesComposition() {
        verifyAnimation("gif", "R0lGODlhDAAIAPAAAP8AAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQAMgAAACwAAAAADAAIAAACCISPqcvtD2MqACH5BAAyAAAALAAAAAAMAAgAgAAA/wAAAAIIhI+py+0PYyoAOw==")
    }
    @Test fun actualWebpStopsWhenPausedAndWhenPreviewLeavesComposition() {
        verifyAnimation("webp", "UklGRsAAAABXRUJQVlA4WAoAAAACAAAACwAABwAAQU5JTQYAAAD/////AABBTk1GSAAAAAAAAAAAAAsAAAcAAPQBAAJWUDggMAAAANABAJ0BKgwACAACADQloAJ0ugH4AAOwAP7wxAv/ILlhdcjX/yA/5Af8gP/48gAAAEFOTUZEAAAAAAAAAAAACwAABwAA9AEAAFZQOCAsAAAAlAEAnQEqDAAIAAAANCWgAnS6AAOYAP75k2//kB//kB//kB//ID/iF3sgMAA=")
    }
    private fun verifyAnimation(extension: String, encoded: String) {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val file = File(context.cacheDir, "preview-animation-test.$extension")
        // Two distinct 500 ms frames with an infinite loop.
        file.writeBytes(android.util.Base64.decode(encoded, android.util.Base64.DEFAULT))
        val playing = mutableStateOf(true)
        val visible = mutableStateOf(true)
        val decoded = AtomicReference<ScaleDrawable?>()
        val loader = (context.applicationContext as MemeDockApplication).container.imageLoader.newBuilder()
            .eventListener(object : EventListener() {
                override fun onSuccess(request: ImageRequest, result: SuccessResult) {
                    decoded.set(result.image.asDrawable(context.resources) as? ScaleDrawable)
                }
            }).build()
        try {
            compose.setContent {
                MemeDockTheme { if (visible.value) StickerPreview(StickerDetails("animation", "动图测试", "", "fixture.$extension", "image/$extension", 12, 8, file.length(), true, false,
                    emptyList(), emptyList(), file.path, null), loader, playing.value) }
            }
            compose.waitUntil(10_000) { decoded.get()?.child is AnimatedImageDrawable }
            val wrapper = requireNotNull(decoded.get())
            val drawable = wrapper.child as AnimatedImageDrawable
            compose.runOnIdle { assertTrue(drawable.isRunning); playing.value = false }
            compose.runOnIdle { assertFalse(drawable.isRunning); playing.value = true }
            compose.runOnIdle { assertTrue(drawable.isRunning); assertNotNull(wrapper.callback) }
            compose.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
            assertFalse(drawable.isRunning)
            compose.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
            compose.runOnIdle { assertTrue(drawable.isRunning); visible.value = false }
            compose.runOnIdle { assertFalse(drawable.isRunning) }
        } finally { loader.shutdown(); file.delete() }
    }
}
