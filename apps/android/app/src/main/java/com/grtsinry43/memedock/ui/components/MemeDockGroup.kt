package com.grtsinry43.memedock.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.ui.theme.MemeDockLayout

class MemeDockGroupScope internal constructor() {
    internal val rows = mutableListOf<@Composable () -> Unit>()
    fun row(content: @Composable () -> Unit) { rows += content }
}

/** A titled, rounded block of rows separated by hairlines. Rows are declared like lazy items. */
@Composable
fun MemeDockGroup(
    modifier: Modifier = Modifier,
    title: String? = null,
    footer: String? = null,
    horizontalPadding: Dp = MemeDockLayout.PagePadding,
    content: MemeDockGroupScope.() -> Unit,
) {
    val rows = MemeDockGroupScope().apply(content).rows
    Column(modifier.fillMaxWidth().padding(horizontal = horizontalPadding)) {
        if (title != null) Text(title, style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 16.dp, bottom = 8.dp).semantics { heading() })
        Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLowest) {
            Column {
                rows.forEachIndexed { index, row ->
                    if (index > 0) HorizontalDivider(Modifier.padding(start = 16.dp), MemeDockLayout.Hairline,
                        MaterialTheme.colorScheme.outlineVariant)
                    row()
                }
            }
        }
        if (footer != null) Text(footer, style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp))
    }
}

/** A chevron appears for plain navigation rows; pass [trailing] for switches, values or custom controls. */
@Composable
fun MemeDockRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    icon: ImageVector? = null,
    value: String? = null,
    destructive: Boolean = false,
    enabled: Boolean = true,
    onClick: (() -> Unit)? = null,
    onLongClick: (() -> Unit)? = null,
    onLongClickLabel: String? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    val colors = MaterialTheme.colorScheme
    val haptics = LocalHapticFeedback.current
    val titleColor = when {
        !enabled -> colors.onSurface.copy(alpha = .38f)
        destructive -> colors.error
        else -> colors.onSurface
    }
    Row(
        modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight)
            .then(when {
                onLongClick != null -> Modifier.combinedClickable(enabled = enabled, role = Role.Button,
                    onLongClickLabel = onLongClickLabel,
                    onLongClick = { haptics.performHapticFeedback(HapticFeedbackType.LongPress); onLongClick() },
                    onClick = { onClick?.invoke() })
                onClick != null -> Modifier.clickable(enabled = enabled, role = Role.Button, onClick = onClick)
                else -> Modifier
            })
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (icon != null) Icon(icon, null, Modifier.size(22.dp),
            tint = if (destructive || !enabled) titleColor else colors.onSurfaceVariant)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = titleColor,
                maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (subtitle != null) Text(subtitle, style = MaterialTheme.typography.bodySmall,
                color = colors.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        if (value != null) Text(value, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant,
            maxLines = 1)
        when {
            trailing != null -> trailing()
            onClick != null && !destructive -> Icon(MemeDockIcons.ChevronRight, null, Modifier.size(20.dp),
                tint = if (enabled) colors.outline else titleColor)
        }
    }
}
