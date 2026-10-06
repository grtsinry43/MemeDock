package com.grtsinry43.memedock.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
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
        Icon(icon, null, Modifier.size(22.dp), tint = if (destructive || !enabled) tint else colors.onSurfaceVariant)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = tint)
            if (supporting != null) Text(supporting, style = MaterialTheme.typography.bodySmall,
                color = colors.onSurfaceVariant)
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
            val colors = MaterialTheme.colorScheme
            // Stays enabled while busy so the spinner keeps the confirm color; clicks are ignored instead.
            Button(
                onClick = { if (!busy) onConfirm() },
                modifier = Modifier.fillMaxWidth().height(52.dp),
                shape = MaterialTheme.shapes.medium,
                colors = if (destructive) ButtonDefaults.buttonColors(containerColor = colors.error, contentColor = colors.onError)
                    else ButtonDefaults.buttonColors(),
            ) {
                if (busy) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp,
                    color = if (destructive) colors.onError else colors.onPrimary)
                else Text(confirmLabel)
            }
            TextButton(onClick = { sheet.dismiss() }, enabled = !busy, modifier = Modifier.fillMaxWidth().height(52.dp),
                shape = MaterialTheme.shapes.medium) {
                Text(stringResource(R.string.cancel), color = colors.onSurfaceVariant)
            }
        }
    }
}
