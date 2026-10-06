package com.grtsinry43.memedock.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

private fun text(size: Int, lineHeight: Int, weight: FontWeight) = TextStyle(
    fontFamily = MemeDockFontFamily, fontWeight = weight,
    fontSize = size.sp, lineHeight = lineHeight.sp, letterSpacing = 0.sp,
)

private val PageTitle = text(28, 36, FontWeight.Bold)
private val SectionTitle = text(20, 28, FontWeight.SemiBold)
private val RowTitle = text(16, 22, FontWeight.Medium)
private val Body = text(14, 20, FontWeight.Normal)
private val Caption = text(12, 16, FontWeight.Normal)

/** Five sizes only; Material slots map onto them so stock components stay on scale. */
val MemeDockTypography = Typography(
    displayLarge = PageTitle,
    displayMedium = PageTitle,
    displaySmall = PageTitle,
    headlineLarge = PageTitle,
    headlineMedium = SectionTitle,
    headlineSmall = SectionTitle,
    titleLarge = SectionTitle,
    titleMedium = RowTitle,
    titleSmall = Body.copy(fontWeight = FontWeight.Medium),
    bodyLarge = RowTitle.copy(fontWeight = FontWeight.Normal),
    bodyMedium = Body,
    bodySmall = Caption,
    labelLarge = Body.copy(fontWeight = FontWeight.Medium),
    labelMedium = Caption.copy(fontWeight = FontWeight.Medium),
    labelSmall = Caption.copy(fontWeight = FontWeight.Medium),
)
