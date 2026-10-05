package com.grtsinry43.memedock.ui.components

import java.text.NumberFormat
import java.util.Locale

fun formatFileSize(bytes: Long, locale: Locale = Locale.getDefault()): String {
    require(bytes >= 0)
    val unit = when {
        bytes < 1024 -> "B"
        bytes < 1024 * 1024 -> "KB"
        bytes < 1024L * 1024 * 1024 -> "MB"
        else -> "GB"
    }
    val divisor = when (unit) { "KB" -> 1024.0; "MB" -> 1024.0 * 1024; "GB" -> 1024.0 * 1024 * 1024; else -> 1.0 }
    return "${NumberFormat.getNumberInstance(locale).apply { maximumFractionDigits = if (unit == "B") 0 else 1 }.format(bytes / divisor)} $unit"
}
