package com.grtsinry43.memedock.ui.components

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.HazeStyle
import dev.chrisbanes.haze.HazeTint
import dev.chrisbanes.haze.hazeEffect
import dev.chrisbanes.haze.hazeSource

/** Content scrolling under the bars; the bars and this source must be siblings, not ancestors. */
fun Modifier.glassSource(state: HazeState): Modifier = hazeSource(state)

/** Blurs on Android 12+; older versions draw the near-opaque fallback tint instead. */
fun Modifier.glass(state: HazeState, style: HazeStyle): Modifier = hazeEffect(state, style)

@Composable
fun glassStyle(): HazeStyle {
    val background = MaterialTheme.colorScheme.background
    return HazeStyle(
        backgroundColor = background,
        tint = HazeTint(background.copy(alpha = .72f)),
        blurRadius = 24.dp,
        noiseFactor = 0f,
        fallbackTint = HazeTint(background.copy(alpha = .95f)),
    )
}
