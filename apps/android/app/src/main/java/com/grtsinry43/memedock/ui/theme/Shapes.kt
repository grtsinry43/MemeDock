package com.grtsinry43.memedock.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.dp

/**
 * extraSmall: badges; small: every control (buttons, chips, fields, search, the selection bar), thumbnails,
 * tile highlights; medium: message bar;
 * large: groups, cards, previews; extraLarge: sheets.
 */
val MemeDockShapes = Shapes(
    extraSmall = RoundedCornerShape(6.dp),
    small = RoundedCornerShape(10.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(20.dp),
    extraLarge = RoundedCornerShape(28.dp),
)
