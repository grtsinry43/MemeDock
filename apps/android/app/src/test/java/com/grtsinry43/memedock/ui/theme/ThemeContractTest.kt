package com.grtsinry43.memedock.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.max
import kotlin.math.min

class ThemeContractTest {
    private fun contrast(foreground: Color, background: Color): Double {
        val first = foreground.luminance().toDouble()
        val second = background.luminance().toDouble()
        return (max(first, second) + 0.05) / (min(first, second) + 0.05)
    }

    private fun assertReadable(name: String, foreground: Color, background: Color) {
        val ratio = contrast(foreground, background)
        assertTrue("$name has insufficient text contrast: $ratio", ratio >= 4.5)
    }

    private fun checkScheme(colors: ColorScheme) {
        with(colors) {
            listOf(
                Triple("primary", onPrimary, primary), Triple("primaryContainer", onPrimaryContainer, primaryContainer),
                Triple("secondary", onSecondary, secondary), Triple("secondaryContainer", onSecondaryContainer, secondaryContainer),
                Triple("tertiary", onTertiary, tertiary), Triple("tertiaryContainer", onTertiaryContainer, tertiaryContainer),
                Triple("error", onError, error), Triple("errorContainer", onErrorContainer, errorContainer),
                Triple("background", onBackground, background), Triple("surface", onSurface, surface),
                Triple("surfaceVariant", onSurfaceVariant, surfaceVariant), Triple("inverseSurface", inverseOnSurface, inverseSurface),
                Triple("primaryFixed", onPrimaryFixed, primaryFixed), Triple("primaryFixedDim", onPrimaryFixedVariant, primaryFixedDim),
                Triple("secondaryFixed", onSecondaryFixed, secondaryFixed), Triple("secondaryFixedDim", onSecondaryFixedVariant, secondaryFixedDim),
                Triple("tertiaryFixed", onTertiaryFixed, tertiaryFixed), Triple("tertiaryFixedDim", onTertiaryFixedVariant, tertiaryFixedDim),
            ).forEach { (name, text, background) -> assertReadable(name, text, background) }
            listOf(surfaceContainerLowest, surfaceContainerLow, surfaceContainer, surfaceContainerHigh,
                surfaceContainerHighest, surfaceBright, surfaceDim).forEach { surface ->
                assertReadable("container", onSurface, surface)
                assertReadable("secondary container text", onSurfaceVariant, surface)
            }
            assertTrue("interactive outline must remain visible", contrast(outline, surface) >= 3.0)
        }
    }

    @Test fun lightAndDarkSchemesKeepTextReadableAcrossAllSurfaceRoles() {
        checkScheme(MemeDockColors.Light)
        checkScheme(MemeDockColors.Dark)
    }

    @Test fun semanticStatusColorsHaveReadableTextInBothThemes() {
        listOf(MemeDockSemanticColors.Light, MemeDockSemanticColors.Dark).forEach {
            assertReadable("success", it.onSuccessContainer, it.successContainer)
            assertReadable("warning", it.onWarningContainer, it.warningContainer)
        }
    }
}
