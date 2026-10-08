package com.grtsinry43.memedock.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import kotlinx.coroutines.launch

class MemeDockSheetScope internal constructor(column: ColumnScope, private val close: () -> Unit) : ColumnScope by column {
    /** Animates the sheet away, then reports the dismissal through `onDismissRequest`. */
    fun dismiss() = close()
}

/**
 * The only modal surface in the app. Keep it composed and drive [visible], so hiding animates
 * instead of the window vanishing. While [dismissible] is false, gestures, back and outside taps
 * cannot close it; use that while an operation started from the sheet is running.
 *
 * [confirmDismiss] sees every user attempt to close the sheet (swipe, back, outside tap, [MemeDockSheetScope.dismiss]).
 * Returning false keeps the sheet open, for example to save first and turn [visible] off afterwards.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MemeDockSheet(
    visible: Boolean,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    title: String? = null,
    subtitle: String? = null,
    dismissible: Boolean = true,
    confirmDismiss: (() -> Boolean)? = null,
    content: @Composable MemeDockSheetScope.() -> Unit,
) {
    val requested by rememberUpdatedState(visible)
    val confirm by rememberUpdatedState(confirmDismiss)
    // Hiding because the caller turned [visible] off is never vetoed.
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true,
        confirmValueChange = { it != SheetValue.Hidden || !requested || confirm?.invoke() != false })
    var shown by remember { mutableStateOf(visible) }
    LaunchedEffect(visible) {
        if (visible) shown = true else if (shown) { state.hide(); shown = false }
    }
    if (!shown) return
    val resources = LocalResources.current
    val scope = rememberCoroutineScope()
    val dismissRequest by rememberUpdatedState(onDismissRequest)
    val close = remember(state) { {
        scope.launch { state.hide() }.invokeOnCompletion { if (!state.isVisible) dismissRequest() }
        Unit
    } }
    ModalBottomSheet(
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        sheetState = state,
        sheetGesturesEnabled = dismissible,
        shape = MaterialTheme.shapes.extraLarge.copy(bottomStart = CornerSize(0), bottomEnd = CornerSize(0)),
        containerColor = MaterialTheme.colorScheme.surfaceContainerLowest,
        contentColor = MaterialTheme.colorScheme.onSurface,
        tonalElevation = 0.dp,
        scrimColor = Color.Black.copy(alpha = .32f),
        dragHandle = { SheetHandle() },
        properties = ModalBottomSheetProperties(shouldDismissOnBackPress = dismissible, shouldDismissOnClickOutside = dismissible),
    ) {
        // The separate window supplies its own Context. Keep app-selected
        // resources for strings without replacing that window's Context.
        CompositionLocalProvider(LocalResources provides resources) {
            if (title != null) {
                Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(title, style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
                    if (subtitle != null) Text(subtitle, style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            MemeDockSheetScope(this, close).content()
            Spacer(Modifier.height(12.dp))
        }
    }
}

/** Keeps the last non-null [value] so a sheet can show it while animating out. */
@Composable
fun <T : Any> rememberRetained(value: T?): T? {
    // Plain holder: a change of value already recomposes the caller, so no snapshot write is needed.
    val retained = remember { Retained<T>() }
    if (value != null) retained.value = value
    return retained.value
}

private class Retained<T : Any> { var value: T? = null }

@Composable
private fun SheetHandle() {
    Box(Modifier.padding(top = 10.dp, bottom = 14.dp).size(width = 36.dp, height = 4.dp)
        .background(MaterialTheme.colorScheme.outlineVariant, CircleShape))
}

@Composable
fun MemeDockSheetAction(
    label: String,
    icon: ImageVector,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    supporting: String? = null,
    destructive: Boolean = false,
    enabled: Boolean = true,
) {
    val colors = MaterialTheme.colorScheme
    val tint = when {
        !enabled -> colors.onSurface.copy(alpha = .38f)
        destructive -> colors.error
        else -> colors.onSurface
    }
    Row(
        modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Icon(icon, null, Modifier.size(MemeDockLayout.IconLarge), tint = if (destructive || !enabled) tint else colors.onSurfaceVariant)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = tint)
            if (supporting != null) Text(supporting, style = MaterialTheme.typography.bodySmall,
                color = colors.onSurfaceVariant)
        }
    }
}

/**
 * One option of a single-choice sheet; the check sits where a [MemeDockSheetAction] has nothing.
 * Its space stays reserved, so text does not rewrap as the selection moves.
 */
@Composable
fun MemeDockSheetChoice(label: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier,
    icon: ImageVector? = null, supporting: String? = null, enabled: Boolean = true) {
    val colors = MaterialTheme.colorScheme
    val tint = if (enabled) colors.onSurface else colors.onSurface.copy(alpha = .38f)
    Row(
        modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight)
            .selectable(selected, enabled = enabled, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        if (icon != null) Icon(icon, null, Modifier.size(MemeDockLayout.IconLarge), tint = if (enabled) colors.onSurfaceVariant else tint)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = tint, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (supporting != null) Text(supporting, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
        }
        if (selected) Icon(MemeDockIcons.Check, null, Modifier.size(MemeDockLayout.IconLarge), tint = colors.primary)
        else Spacer(Modifier.size(MemeDockLayout.IconLarge))
    }
}

/** Stands in for sheet content that is still loading. */
@Composable
fun MemeDockSheetLoading(modifier: Modifier = Modifier) {
    Column(modifier.padding(horizontal = 24.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        MemeDockSkeleton(Modifier.fillMaxWidth(.6f).height(20.dp), MaterialTheme.shapes.extraSmall)
        MemeDockSkeleton(Modifier.fillMaxWidth(.4f).height(20.dp), MaterialTheme.shapes.extraSmall)
    }
}

@Composable
fun MemeDockSheetHint(text: String, modifier: Modifier = Modifier, error: Boolean = false) {
    Text(text, modifier.padding(horizontal = 24.dp, vertical = 8.dp), style = MaterialTheme.typography.bodyMedium,
        color = if (error) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
}

/** A failure line; [retry] adds the action at its end. [inset] defaults to the sheet's. */
@Composable
fun MemeDockSheetError(text: String, modifier: Modifier = Modifier, inset: Dp = 24.dp, retryLabel: String? = null,
    retry: (() -> Unit)? = null) {
    Row(modifier.fillMaxWidth().padding(horizontal = inset).heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.GapSmall)) {
        Text(text, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
        // Shifted by its own padding so the label, not the button edge, lines up with the inset.
        if (retry != null) TextButton(retry, Modifier.offset(x = 12.dp), contentPadding = PaddingValues(horizontal = 12.dp)) {
            Text(retryLabel ?: stringResource(R.string.retry))
        }
    }
}

/** Replaces confirmation dialogs; the caller hides it by turning [visible] off when the work is done. */
@Composable
fun MemeDockConfirmSheet(
    visible: Boolean,
    title: String,
    message: String,
    confirmLabel: String,
    onConfirm: () -> Unit,
    onDismissRequest: () -> Unit,
    destructive: Boolean = false,
    busy: Boolean = false,
) {
    MemeDockSheet(visible, onDismissRequest, title = title, subtitle = message, dismissible = !busy) {
        val sheet = this
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(top = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)) {
            MemeDockButton(confirmLabel, onConfirm, busy = busy, destructive = destructive)
            MemeDockTextButton(stringResource(R.string.cancel), { sheet.dismiss() }, enabled = !busy)
        }
    }
}
