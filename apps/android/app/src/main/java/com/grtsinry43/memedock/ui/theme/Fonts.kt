package com.grtsinry43.memedock.ui.theme

import android.content.Context
import android.graphics.Typeface
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.font.AndroidFont
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontLoadingStrategy
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import com.grtsinry43.memedock.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private val weights = listOf(FontWeight.Normal, FontWeight.Medium, FontWeight.SemiBold, FontWeight.Bold)

@OptIn(ExperimentalTextApi::class)
val MemeDockChineseFontFamily = FontFamily(weights.map { weight ->
    Font(R.font.memedock_noto_sans_sc, weight = weight, loadingStrategy = FontLoadingStrategy.Async,
        variationSettings = FontVariation.Settings(FontVariation.weight(weight.weight)))
})

@OptIn(ExperimentalTextApi::class)
val MemeDockEnglishFontFamily = FontFamily(weights.map { weight ->
    Font(R.font.memedock_quicksand, weight = weight, loadingStrategy = FontLoadingStrategy.Async,
        variationSettings = FontVariation.Settings(FontVariation.weight(weight.weight)))
})

// A Compose font list selects a weight; it does not provide per-glyph fallback.
// Android 9 uses bundled Noto Sans SC; Android 10+ supplies a custom fallback chain.
val MemeDockFontFamily = if (Build.VERSION.SDK_INT >= 29) {
    FontFamily(weights.map(::MemeDockMixedFont))
} else {
    MemeDockChineseFontFamily
}

private data class MemeDockMixedFont(override val weight: FontWeight) : AndroidFont(
    FontLoadingStrategy.Async,
    MixedFontLoader,
    FontVariation.Settings(FontVariation.weight(weight.weight)),
) {
    override val style = FontStyle.Normal
}

private object MixedFontLoader : AndroidFont.TypefaceLoader {
    // Compose uses awaitLoad for Async fonts; the synchronous path supports tooling.
    override fun loadBlocking(context: Context, font: AndroidFont): Typeface = load(context, font)

    override suspend fun awaitLoad(context: Context, font: AndroidFont): Typeface =
        withContext(Dispatchers.IO) { load(context, font) }

    private fun load(context: Context, font: AndroidFont): Typeface {
        return if (Build.VERSION.SDK_INT >= 29) {
            loadApi29(context, font.weight.weight)
        } else {
            error("Custom font fallback requires Android 10")
        }
    }

    @RequiresApi(29)
    private fun loadApi29(context: Context, weight: Int): Typeface {
        fun family(resource: Int) = android.graphics.fonts.FontFamily.Builder(
            android.graphics.fonts.Font.Builder(context.resources, resource)
                .setWeight(weight).setFontVariationSettings("'wght' $weight").build(),
        ).build()
        return Typeface.CustomFallbackBuilder(family(R.font.memedock_quicksand))
            .addCustomFallback(family(R.font.memedock_noto_sans_sc))
            .setSystemFallback("sans-serif")
            .setStyle(android.graphics.fonts.FontStyle(weight, android.graphics.fonts.FontStyle.FONT_SLANT_UPRIGHT))
            .build()
    }
}
