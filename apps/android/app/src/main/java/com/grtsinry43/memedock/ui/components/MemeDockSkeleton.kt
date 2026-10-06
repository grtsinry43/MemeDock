package com.grtsinry43.memedock.ui.components

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer

/** A breathing placeholder block; size it with [modifier]. The pulse is read in the draw phase only. */
@Composable
fun MemeDockSkeleton(modifier: Modifier = Modifier, shape: Shape = MaterialTheme.shapes.medium) {
    val pulse = rememberInfiniteTransition(label = "skeleton")
    val alpha by pulse.animateFloat(.55f, 1f, infiniteRepeatable(tween(900), RepeatMode.Reverse), label = "skeleton-alpha")
    Box(modifier.graphicsLayer { this.alpha = alpha }.background(MaterialTheme.colorScheme.surfaceContainer, shape))
}
