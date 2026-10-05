package com.grtsinry43.memedock.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

private fun text(size: Int, lineHeight: Int, weight: FontWeight, spacing: Float = 0f) = TextStyle(
    fontFamily = MemeDockFontFamily, fontWeight = weight,
    fontSize = size.sp, lineHeight = lineHeight.sp, letterSpacing = spacing.sp,
)

val MemeDockTypography = Typography(
    displayLarge = text(57, 64, FontWeight.Bold, -0.25f),
    displayMedium = text(45, 52, FontWeight.Bold),
    displaySmall = text(36, 44, FontWeight.SemiBold),
    headlineLarge = text(32, 40, FontWeight.Bold),
    headlineMedium = text(28, 36, FontWeight.SemiBold),
    headlineSmall = text(24, 32, FontWeight.SemiBold),
    titleLarge = text(22, 28, FontWeight.SemiBold),
    titleMedium = text(16, 24, FontWeight.Medium, 0.15f),
    titleSmall = text(14, 20, FontWeight.Medium, 0.1f),
    bodyLarge = text(16, 24, FontWeight.Normal, 0.5f),
    bodyMedium = text(14, 20, FontWeight.Normal, 0.25f),
    bodySmall = text(12, 16, FontWeight.Normal, 0.4f),
    labelLarge = text(14, 20, FontWeight.Medium, 0.1f),
    labelMedium = text(12, 16, FontWeight.Medium, 0.5f),
    labelSmall = text(11, 16, FontWeight.Medium, 0.5f),
)
