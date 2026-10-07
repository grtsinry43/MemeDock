package com.grtsinry43.memedock.ui.theme

import androidx.compose.animation.core.CubicBezierEasing

object MemeDockMotion {
    const val Page = 300
    const val PageReturn = 250
    const val Tab = 200
    const val Fade = 200
    const val SharedImage = 300
    const val Feedback = 180
    val Smooth = CubicBezierEasing(0.2f, 0f, 0f, 1f)
    val Rebound = CubicBezierEasing(0.34f, 1.18f, 0.64f, 1f)
}
