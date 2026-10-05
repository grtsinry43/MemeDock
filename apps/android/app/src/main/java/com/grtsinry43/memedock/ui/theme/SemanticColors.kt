package com.grtsinry43.memedock.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

@Immutable
data class MemeDockSemanticColors(
    val successContainer: Color,
    val onSuccessContainer: Color,
    val warningContainer: Color,
    val onWarningContainer: Color,
) {
    companion object {
        val Light = MemeDockSemanticColors(
            Color(0xFFDCFCE7), Color(0xFF14532D), Color(0xFFFEF3C7), Color(0xFF713F12),
        )
        val Dark = MemeDockSemanticColors(
            Color(0xFF123B25), Color(0xFFABEFBE), Color(0xFF503B12), Color(0xFFFFE09A),
        )
    }
}

val LocalMemeDockSemanticColors = staticCompositionLocalOf { MemeDockSemanticColors.Light }
