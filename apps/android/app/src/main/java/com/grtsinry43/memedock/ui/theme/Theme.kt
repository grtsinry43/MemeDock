package com.grtsinry43.memedock.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFontFamilyResolver

@Composable
fun MemeDockTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit,
) {
    val context = LocalContext.current
    val colors = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        darkTheme -> MemeDockColors.Dark
        else -> MemeDockColors.Light
    }
    val resolver = LocalFontFamilyResolver.current
    LaunchedEffect(resolver) { resolver.preload(MemeDockFontFamily) }
    CompositionLocalProvider(LocalMemeDockSemanticColors provides
        if (darkTheme) MemeDockSemanticColors.Dark else MemeDockSemanticColors.Light) {
        MaterialTheme(colorScheme = colors, typography = MemeDockTypography, shapes = MemeDockShapes) {
            // Pages draw on the window background rather than a Surface.
            CompositionLocalProvider(LocalContentColor provides colors.onBackground, content = content)
        }
    }
}
