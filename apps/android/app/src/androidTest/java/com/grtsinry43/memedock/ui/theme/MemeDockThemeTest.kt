package com.grtsinry43.memedock.ui.theme

import android.content.res.Configuration
import android.graphics.Paint
import android.graphics.Typeface
import android.os.Build
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MemeDockThemeTest {
    @get:Rule val compose = createComposeRule()

    @Test fun themeTracksSystemDarkModeAndDynamicColorIsAnExplicitChoice() {
        val dark = mutableStateOf(false)
        val dynamic = mutableStateOf(false)
        lateinit var actual: ColorScheme
        lateinit var semantic: MemeDockSemanticColors
        compose.setContent {
            val configuration = Configuration(LocalConfiguration.current).apply {
                uiMode = (uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or
                    if (dark.value) Configuration.UI_MODE_NIGHT_YES else Configuration.UI_MODE_NIGHT_NO
            }
            CompositionLocalProvider(LocalConfiguration provides configuration) {
                MemeDockTheme(dynamicColor = dynamic.value) {
                    val colors = MaterialTheme.colorScheme
                    val status = LocalMemeDockSemanticColors.current
                    SideEffect { actual = colors; semantic = status }
                    Text("MemeDock 贴纸库 123", style = MaterialTheme.typography.bodyLarge)
                }
            }
        }
        compose.runOnIdle {
            assertEquals(MemeDockColors.Light.primary, actual.primary)
            assertEquals(MemeDockSemanticColors.Light, semantic)
            dark.value = true
        }
        compose.runOnIdle {
            assertEquals(MemeDockColors.Dark.primary, actual.primary)
            assertEquals(MemeDockSemanticColors.Dark, semantic)
            dynamic.value = true
        }
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        compose.runOnIdle {
            val expected = if (Build.VERSION.SDK_INT >= 31) dynamicDarkColorScheme(context) else MemeDockColors.Dark
            assertEquals(expected.primary, actual.primary)
            assertEquals(expected.surfaceContainer, actual.surfaceContainer)
            dark.value = false
        }
        compose.runOnIdle {
            val expected = if (Build.VERSION.SDK_INT >= 31) dynamicLightColorScheme(context) else MemeDockColors.Light
            assertEquals(expected.primary, actual.primary)
        }
    }

    @Test fun mixedFontActuallyUsesQuicksandForLatinAndNotoForChineseAtEveryWeight() = runBlocking {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val resolver = createFontFamilyResolver(context)
        listOf(MemeDockFontFamily, MemeDockEnglishFontFamily, MemeDockChineseFontFamily).forEach { resolver.preload(it) }
        val weights = listOf(FontWeight.Normal, FontWeight.Medium, FontWeight.SemiBold, FontWeight.Bold)
        for (weight in weights) {
            fun typeface(family: FontFamily) = resolver.resolve(family, weight).value as Typeface
            val mixed = typeface(MemeDockFontFamily)
            val latin = typeface(MemeDockEnglishFontFamily)
            val chinese = typeface(MemeDockChineseFontFamily)
            fun advance(font: Typeface, text: String) = Paint().apply { typeface = font; textSize = 100f }.measureText(text)
            assertEquals(weight.weight, mixed.weight)
            val expectedLatin = if (Build.VERSION.SDK_INT >= 29) latin else chinese
            assertEquals(advance(expectedLatin, "MemeDock 012345"), advance(mixed, "MemeDock 012345"), 0.01f)
            assertEquals(advance(chinese, "贴纸库你好世界"), advance(mixed, "贴纸库你好世界"), 0.01f)
            assertTrue(Paint().apply { typeface = mixed }.hasGlyph("贴"))
        }
    }
}
