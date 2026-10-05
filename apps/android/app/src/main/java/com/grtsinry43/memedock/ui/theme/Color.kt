package com.grtsinry43.memedock.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color

object MemeDockColors {
    val BrandBlue = Color(0xFF2E7CF6)

    // Deeper action colors preserve the palette and readable white labels.
    val Light = lightColorScheme(
        primary = Color(0xFF2168D5), onPrimary = Color.White,
        primaryContainer = Color(0xFFE3F2FF), onPrimaryContainer = Color(0xFF001D35),
        inversePrimary = Color(0xFF5B9FFF),
        secondary = Color(0xFF5B9FFF), onSecondary = Color(0xFF001B3D),
        secondaryContainer = Color(0xFFEBF4FF), onSecondaryContainer = Color(0xFF001B3D),
        tertiary = Color(0xFF7055CE), onTertiary = Color.White,
        tertiaryContainer = Color(0xFFEDE7FF), onTertiaryContainer = Color(0xFF23036A),
        error = Color(0xFFBA1A1A), onError = Color.White,
        errorContainer = Color(0xFFFFDAD6), onErrorContainer = Color(0xFF410002),
        background = Color(0xFFFAFBFC), onBackground = Color(0xFF1A1C1E),
        surface = Color.White, onSurface = Color(0xFF1A1C1E),
        surfaceVariant = Color(0xFFF5F7FA), onSurfaceVariant = Color(0xFF535D6B),
        surfaceTint = Color(0xFF2168D5),
        inverseSurface = Color(0xFF1A1F29), inverseOnSurface = Color(0xFFE4E6E9),
        outline = Color(0xFF687386), outlineVariant = Color(0xFFE5E7EB), scrim = Color.Black,
        surfaceBright = Color.White, surfaceDim = Color(0xFFD9DEE7),
        surfaceContainerLowest = Color.White, surfaceContainerLow = Color(0xFFF6F8FC),
        surfaceContainer = Color(0xFFEFF3F8), surfaceContainerHigh = Color(0xFFE8EDF4),
        surfaceContainerHighest = Color(0xFFE1E7EF),
        primaryFixed = Color(0xFFE3F2FF), primaryFixedDim = Color(0xFFB4D4FF),
        onPrimaryFixed = Color(0xFF001D35), onPrimaryFixedVariant = Color(0xFF0B4BA3),
        secondaryFixed = Color(0xFFEBF4FF), secondaryFixedDim = Color(0xFFB9D8FF),
        onSecondaryFixed = Color(0xFF001B3D), onSecondaryFixedVariant = Color(0xFF0D3D6E),
        tertiaryFixed = Color(0xFFEDE7FF), tertiaryFixedDim = Color(0xFFCDBEFF),
        onTertiaryFixed = Color(0xFF23036A), onTertiaryFixedVariant = Color(0xFF3E2D7A),
    )

    val Dark = darkColorScheme(
        primary = Color(0xFF5B9FFF), onPrimary = Color(0xFF001D35),
        primaryContainer = Color(0xFF0B4BA3), onPrimaryContainer = Color(0xFFE3F2FF),
        inversePrimary = Color(0xFF2168D5),
        secondary = Color(0xFF7CB5FF), onSecondary = Color(0xFF001B3D),
        secondaryContainer = Color(0xFF0D3D6E), onSecondaryContainer = Color(0xFFEBF4FF),
        tertiary = Color(0xFF9B8AFF), onTertiary = Color(0xFF23036A),
        tertiaryContainer = Color(0xFF3E2D7A), onTertiaryContainer = Color(0xFFEDE7FF),
        error = Color(0xFFFFB4AB), onError = Color(0xFF690005),
        errorContainer = Color(0xFF8C1D18), onErrorContainer = Color(0xFFFFEBEE),
        background = Color(0xFF0F1419), onBackground = Color(0xFFE4E6E9),
        surface = Color(0xFF1A1F29), onSurface = Color(0xFFE4E6E9),
        surfaceVariant = Color(0xFF141820), onSurfaceVariant = Color(0xFFB4B8BE),
        surfaceTint = Color(0xFF5B9FFF),
        inverseSurface = Color(0xFFE4E6E9), inverseOnSurface = Color(0xFF1A1C1E),
        outline = Color(0xFF8893A5), outlineVariant = Color(0xFF2D3748), scrim = Color.Black,
        surfaceBright = Color(0xFF343B48), surfaceDim = Color(0xFF0F1419),
        surfaceContainerLowest = Color(0xFF0B1015), surfaceContainerLow = Color(0xFF141A23),
        surfaceContainer = Color(0xFF1A1F29), surfaceContainerHigh = Color(0xFF242B36),
        surfaceContainerHighest = Color(0xFF303845),
        primaryFixed = Light.primaryFixed, primaryFixedDim = Light.primaryFixedDim,
        onPrimaryFixed = Light.onPrimaryFixed, onPrimaryFixedVariant = Light.onPrimaryFixedVariant,
        secondaryFixed = Light.secondaryFixed, secondaryFixedDim = Light.secondaryFixedDim,
        onSecondaryFixed = Light.onSecondaryFixed, onSecondaryFixedVariant = Light.onSecondaryFixedVariant,
        tertiaryFixed = Light.tertiaryFixed, tertiaryFixedDim = Light.tertiaryFixedDim,
        onTertiaryFixed = Light.onTertiaryFixed, onTertiaryFixedVariant = Light.onTertiaryFixedVariant,
    )
}
