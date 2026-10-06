package com.grtsinry43.memedock.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color

/**
 * Surface roles: `background` is the page canvas, `surfaceContainerLowest` is raised content
 * (groups, sheets, message bar), `surfaceContainer` fills inputs and chips, `surfaceContainerHigh`
 * marks pressed or selected neutrals. `tertiary` is reserved for stars.
 */
object MemeDockColors {
    val BrandBlue = Color(0xFF2E7CF6)

    val Light = lightColorScheme(
        primary = Color(0xFF1F6FEB), onPrimary = Color.White,
        primaryContainer = Color(0xFFE6F0FF), onPrimaryContainer = Color(0xFF0B3A82),
        inversePrimary = Color(0xFF8AB8FF),
        secondary = Color(0xFF5C6370), onSecondary = Color.White,
        secondaryContainer = Color(0xFFE9ECF0), onSecondaryContainer = Color(0xFF1D2128),
        tertiary = Color(0xFFE29A00), onTertiary = Color(0xFF241A00),
        tertiaryContainer = Color(0xFFFFF1CC), onTertiaryContainer = Color(0xFF3D2C00),
        error = Color(0xFFD93025), onError = Color.White,
        errorContainer = Color(0xFFFDE7E5), onErrorContainer = Color(0xFF5F110B),
        background = Color(0xFFF6F7F9), onBackground = Color(0xFF16181C),
        surface = Color(0xFFF6F7F9), onSurface = Color(0xFF16181C),
        surfaceVariant = Color(0xFFEBEDF1), onSurfaceVariant = Color(0xFF565D68),
        surfaceTint = Color.Transparent,
        inverseSurface = Color(0xFF24272C), inverseOnSurface = Color(0xFFF1F2F4),
        outline = Color(0xFF848B96), outlineVariant = Color(0xFFE1E4E9), scrim = Color.Black,
        surfaceBright = Color.White, surfaceDim = Color(0xFFDDE0E5),
        surfaceContainerLowest = Color.White, surfaceContainerLow = Color(0xFFF0F2F5),
        surfaceContainer = Color(0xFFEBEDF1), surfaceContainerHigh = Color(0xFFE2E5EA),
        surfaceContainerHighest = Color(0xFFD9DDE3),
        primaryFixed = Color(0xFFE6F0FF), primaryFixedDim = Color(0xFFB7D2FF),
        onPrimaryFixed = Color(0xFF0B3A82), onPrimaryFixedVariant = Color(0xFF1652B8),
        secondaryFixed = Color(0xFFE9ECF0), secondaryFixedDim = Color(0xFFCDD2D9),
        onSecondaryFixed = Color(0xFF1D2128), onSecondaryFixedVariant = Color(0xFF444B56),
        tertiaryFixed = Color(0xFFFFF1CC), tertiaryFixedDim = Color(0xFFFFD873),
        onTertiaryFixed = Color(0xFF3D2C00), onTertiaryFixedVariant = Color(0xFF5C4300),
    )

    val Dark = darkColorScheme(
        primary = Color(0xFF6EA8FF), onPrimary = Color(0xFF002A66),
        primaryContainer = Color(0xFF15315C), onPrimaryContainer = Color(0xFFD6E5FF),
        inversePrimary = Color(0xFF1F6FEB),
        secondary = Color(0xFFB4BAC4), onSecondary = Color(0xFF1D2128),
        secondaryContainer = Color(0xFF2C3037), onSecondaryContainer = Color(0xFFE8EAED),
        tertiary = Color(0xFFFFC94D), onTertiary = Color(0xFF3D2C00),
        tertiaryContainer = Color(0xFF4A3800), onTertiaryContainer = Color(0xFFFFE7A8),
        error = Color(0xFFFF8A80), onError = Color(0xFF5F110B),
        errorContainer = Color(0xFF4A1512), onErrorContainer = Color(0xFFFFDAD6),
        background = Color(0xFF0F1114), onBackground = Color(0xFFE8EAED),
        surface = Color(0xFF0F1114), onSurface = Color(0xFFE8EAED),
        surfaceVariant = Color(0xFF23272D), onSurfaceVariant = Color(0xFFA4AAB4),
        surfaceTint = Color.Transparent,
        inverseSurface = Color(0xFFE8EAED), inverseOnSurface = Color(0xFF1A1D21),
        outline = Color(0xFF6F7680), outlineVariant = Color(0xFF2A2E35), scrim = Color.Black,
        surfaceBright = Color(0xFF353A42), surfaceDim = Color(0xFF0F1114),
        // Dark surfaces rise by getting lighter, so "lowest" is the raised layer here too.
        surfaceContainerLowest = Color(0xFF181B20), surfaceContainerLow = Color(0xFF1C1F24),
        surfaceContainer = Color(0xFF23272D), surfaceContainerHigh = Color(0xFF2C3037),
        surfaceContainerHighest = Color(0xFF353A42),
        primaryFixed = Light.primaryFixed, primaryFixedDim = Light.primaryFixedDim,
        onPrimaryFixed = Light.onPrimaryFixed, onPrimaryFixedVariant = Light.onPrimaryFixedVariant,
        secondaryFixed = Light.secondaryFixed, secondaryFixedDim = Light.secondaryFixedDim,
        onSecondaryFixed = Light.onSecondaryFixed, onSecondaryFixedVariant = Light.onSecondaryFixedVariant,
        tertiaryFixed = Light.tertiaryFixed, tertiaryFixedDim = Light.tertiaryFixedDim,
        onTertiaryFixed = Light.onTertiaryFixed, onTertiaryFixedVariant = Light.onTertiaryFixedVariant,
    )
}
