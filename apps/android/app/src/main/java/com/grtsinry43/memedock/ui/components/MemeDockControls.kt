package com.grtsinry43.memedock.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.integerResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.HazeTint

/**
 * Large title of a tab root page; [actions] are icon buttons whose visual edge lands on the page padding. There is
 * no bottom padding: the 4 dp an icon button keeps below its circle is the start of the gap to the content.
 */
@Composable
fun MemeDockPageHeader(title: String, modifier: Modifier = Modifier, actions: @Composable RowScope.() -> Unit = {}) {
    Row(
        modifier.fillMaxWidth()
            .padding(start = MemeDockLayout.PagePadding, end = MemeDockLayout.GapMedium, top = MemeDockLayout.GapSmall)
            .heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, Modifier.weight(1f).semantics { heading() },
            style = MaterialTheme.typography.headlineLarge.copy(fontWeight = FontWeight(integerResource(R.integer.page_title_weight))),
            maxLines = 1, overflow = TextOverflow.Ellipsis)
        actions()
    }
}

/**
 * Label above a block of content. [inset] lines the label up with the text of that content: page padding for
 * content placed on the page, page padding plus the row inset above a [MemeDockGroup], 24 dp in sheets.
 */
@Composable
fun MemeDockSectionHeader(
    title: String,
    modifier: Modifier = Modifier,
    inset: Dp = MemeDockLayout.PagePadding,
    action: String? = null,
    actionEnabled: Boolean = true,
    onAction: (() -> Unit)? = null,
) {
    Row(
        modifier.fillMaxWidth().padding(start = inset, end = inset, bottom = MemeDockLayout.GapXSmall)
            .heightIn(min = 32.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, Modifier.weight(1f).semantics { heading() }, style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        // Shifted by its own padding so the label, not the button edge, lines up with the inset.
        if (action != null && onAction != null) TextButton(onClick = onAction, Modifier.offset(x = 12.dp).heightIn(min = 32.dp),
            enabled = actionEnabled,
            contentPadding = PaddingValues(horizontal = 12.dp)) {
            Text(action, style = MaterialTheme.typography.labelLarge)
        }
    }
}

/**
 * Full-width primary action. While [busy] the button keeps its colors and ignores clicks; a spinner replaces
 * [icon] and [busyText], when given, replaces [text].
 */
@Composable
fun MemeDockButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    busy: Boolean = false,
    busyText: String? = null,
    icon: ImageVector? = null,
    destructive: Boolean = false,
) {
    val colors = MaterialTheme.colorScheme
    val content = if (destructive) colors.onError else colors.onPrimary
    Button(
        onClick = { if (!busy) onClick() },
        modifier = modifier.fillMaxWidth().height(MemeDockLayout.ButtonHeight),
        enabled = enabled,
        shape = MaterialTheme.shapes.small,
        colors = if (destructive) ButtonDefaults.buttonColors(containerColor = colors.error, contentColor = colors.onError)
            else ButtonDefaults.buttonColors(),
    ) {
        if (busy) CircularProgressIndicator(Modifier.size(MemeDockLayout.IconMedium), color = content, strokeWidth = 2.dp)
        else if (icon != null) Icon(icon, null, Modifier.size(MemeDockLayout.IconMedium))
        val label = if (busy) busyText else text
        if (label != null) {
            if (busy || icon != null) Spacer(Modifier.width(MemeDockLayout.GapSmall))
            Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** Full-width secondary action placed under a [MemeDockButton]. */
@Composable
fun MemeDockTextButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    destructive: Boolean = false,
) {
    TextButton(onClick, modifier.fillMaxWidth().height(MemeDockLayout.ButtonHeight), enabled = enabled,
        shape = MaterialTheme.shapes.small) {
        Text(text, color = when {
            !enabled -> Color.Unspecified
            destructive -> MaterialTheme.colorScheme.error
            else -> MaterialTheme.colorScheme.onSurfaceVariant
        }, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/**
 * A chip that opens or describes something. [container] contrasts with the surface beneath: surfaceContainer
 * on sheets and groups, surfaceContainerLowest on the page background. [accent] marks the chip as active.
 */
@Composable
fun MemeDockChip(
    label: String,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    accent: Boolean = false,
    enabled: Boolean = true,
    container: Color = MaterialTheme.colorScheme.surfaceContainer,
    onLongClick: (() -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    val haptics = LocalHapticFeedback.current
    val longClickLabel = onLongClick?.let { stringResource(R.string.more_actions) }
    ChipBody(label, icon, accent, enabled, container, modifier, when {
        onLongClick != null -> Modifier.combinedClickable(enabled = enabled, role = Role.Button, onLongClickLabel = longClickLabel,
            onLongClick = { haptics.performHapticFeedback(HapticFeedbackType.LongPress); onLongClick() },
            onClick = { onClick?.invoke() })
        onClick != null -> Modifier.clickable(enabled = enabled, role = Role.Button, onClick = onClick)
        else -> Modifier
    })
}

/** The accent chip that starts creating an item next to the chips of its kind. */
@Composable
fun MemeDockAddChip(label: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) =
    MemeDockChip(label, modifier, MemeDockIcons.Add, accent = true, enabled = enabled, onClick = onClick)

/** A chip in a single ([multiple] false) or multiple choice; selected chips carry a check. */
@Composable
fun MemeDockChoiceChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    multiple: Boolean = false,
    enabled: Boolean = true,
) {
    ChipBody(label, if (selected) MemeDockIcons.Check else null, selected, enabled, MaterialTheme.colorScheme.surfaceContainer,
        modifier,
        if (multiple) Modifier.toggleable(selected, enabled = enabled, role = Role.Checkbox) { onClick() }
        else Modifier.selectable(selected, enabled = enabled, role = Role.RadioButton, onClick = onClick))
}

@Composable
private fun ChipBody(label: String, icon: ImageVector?, accent: Boolean, enabled: Boolean, container: Color,
    modifier: Modifier, interaction: Modifier) {
    val colors = MaterialTheme.colorScheme
    val content = when {
        !enabled -> colors.onSurface.copy(alpha = .38f)
        accent -> colors.primary
        else -> colors.onSurface
    }
    Row(
        modifier.heightIn(min = MemeDockLayout.ChipHeight).clip(MaterialTheme.shapes.small)
            .background(if (accent) colors.primary.copy(alpha = .12f) else container)
            .then(interaction)
            .padding(start = if (icon != null) 12.dp else 14.dp, end = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (icon != null) Icon(icon, null, Modifier.size(MemeDockLayout.IconSmall),
            tint = if (accent || !enabled) content else colors.onSurfaceVariant)
        Text(label, style = MaterialTheme.typography.bodyMedium, color = content, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** Single-line form field on the same filled surface as search and inline edits. */
@Composable
fun MemeDockTextField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    placeholder: String? = null,
    trailing: (@Composable () -> Unit)? = null,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
) {
    val container = MaterialTheme.colorScheme.surfaceContainer
    TextField(value, onValueChange, modifier.fillMaxWidth(), enabled = enabled, singleLine = true,
        label = { Text(label) }, placeholder = placeholder?.let { { Text(it) } }, trailingIcon = trailing,
        visualTransformation = visualTransformation, keyboardOptions = keyboardOptions, shape = MaterialTheme.shapes.small,
        colors = TextFieldDefaults.colors(
            focusedContainerColor = container, unfocusedContainerColor = container, disabledContainerColor = container,
            focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent,
            disabledIndicatorColor = Color.Transparent, errorIndicatorColor = Color.Transparent,
        ))
}

/** Bar floating over scrolling content near the bottom edge, such as the selection bar. */
@Composable
fun MemeDockFloatingBar(glass: HazeState, modifier: Modifier = Modifier, content: @Composable RowScope.() -> Unit) {
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    Row(
        modifier.widthIn(max = 560.dp).fillMaxWidth().heightIn(min = MemeDockLayout.ButtonHeight).shadow(1.dp, shape).clip(shape)
            .glass(glass, glassStyle().copy(tints = listOf(HazeTint(colors.background.copy(alpha = .88f)))))
            .border(1.dp, colors.outlineVariant.copy(alpha = .5f), shape)
            .padding(start = 20.dp, end = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.GapSmall),
        content = content,
    )
}

/** Selection state drawn over a sticker; the ring stays visible on both light and dark artwork. */
@Composable
fun MemeDockSelectionMark(selected: Boolean, modifier: Modifier = Modifier, contentDescription: String? = null) {
    val colors = MaterialTheme.colorScheme
    Box(
        modifier.size(MemeDockLayout.IconLarge)
            .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier)
            .then(
            if (selected) Modifier.background(colors.primary, CircleShape)
            else Modifier.background(Color.Black.copy(alpha = .16f), CircleShape).border(1.5.dp, Color.White, CircleShape),
        ),
        contentAlignment = Alignment.Center,
    ) {
        if (selected) Icon(MemeDockIcons.Check, null, Modifier.size(MemeDockLayout.IconSmall), tint = colors.onPrimary)
    }
}
