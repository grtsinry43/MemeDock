package com.grtsinry43.memedock.ui.components

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.KeyboardActionHandler
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.theme.MemeDockLayout

/**
 * Edits text in place. The keyboard action or leaving the field commits; back cancels. A commit
 * that changes nothing, or blanks a value that may not be empty, cancels instead. While [busy] the
 * text is read-only; if the caller reports [error], editing resumes so the user can fix it.
 */
@Composable
fun MemeDockInlineEdit(
    initial: String,
    placeholder: String,
    onCommit: (String) -> Unit,
    onCancel: () -> Unit,
    modifier: Modifier = Modifier,
    busy: Boolean = false,
    error: String? = null,
    allowEmpty: Boolean = false,
    multiline: Boolean = false,
) {
    val state = rememberTextFieldState(initial)
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    var submitted by remember { mutableStateOf(false) }
    fun finish() {
        if (submitted || busy) return
        submitted = true
        val text = state.text.toString().trim()
        if (text == initial.trim() || (text.isEmpty() && !allowEmpty)) onCancel() else onCommit(text)
    }
    LaunchedEffect(Unit) { focus.requestFocus() }
    LaunchedEffect(error) { if (error != null) { submitted = false; focus.requestFocus() } }
    BackHandler(enabled = !busy) { submitted = true; onCancel() }
    val colors = MaterialTheme.colorScheme
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        BasicTextField(
            state = state,
            modifier = Modifier.fillMaxWidth().focusRequester(focus).onFocusChanged {
                if (focused && !it.isFocused) finish()
                focused = it.isFocused
            },
            readOnly = busy,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = colors.onSurface),
            cursorBrush = SolidColor(colors.primary),
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None,
                imeAction = if (multiline) ImeAction.Default else ImeAction.Done),
            onKeyboardAction = if (multiline) null else KeyboardActionHandler { finish() },
            lineLimits = if (multiline) TextFieldLineLimits.MultiLine(1, 6) else TextFieldLineLimits.SingleLine,
            decorator = { field ->
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 48.dp)
                        .background(if (error != null) colors.errorContainer else colors.surfaceContainer, MaterialTheme.shapes.small)
                        .padding(start = 14.dp, end = 6.dp, top = 12.dp, bottom = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.weight(1f)) {
                        if (state.text.isEmpty()) Text(placeholder, style = MaterialTheme.typography.bodyLarge,
                            color = colors.onSurfaceVariant)
                        field()
                    }
                    when {
                        busy -> CircularProgressIndicator(Modifier.padding(horizontal = 6.dp).size(18.dp), strokeWidth = 2.dp)
                        // The return key adds a line here, so the field needs its own way to finish.
                        multiline -> Icon(MemeDockIcons.Check, stringResource(R.string.done),
                            Modifier.size(28.dp).clickable(role = Role.Button) { finish() }.padding(4.dp), tint = colors.primary)
                        state.text.isNotEmpty() -> Icon(MemeDockIcons.Close, stringResource(R.string.clear),
                            Modifier.size(28.dp).clickable(role = Role.Button) { state.edit { replace(0, length, "") } }
                                .padding(5.dp), tint = colors.onSurfaceVariant)
                    }
                }
            },
        )
        if (error != null) Text(error, style = MaterialTheme.typography.bodySmall, color = colors.error,
            modifier = Modifier.padding(horizontal = 4.dp))
    }
}

/** An "add" row that becomes an inline field; the caller owns [editing] and leaves it once creation succeeds. */
@Composable
fun MemeDockInlineCreate(
    label: String,
    placeholder: String,
    editing: Boolean,
    onStart: () -> Unit,
    onCreate: (String) -> Unit,
    onCancel: () -> Unit,
    modifier: Modifier = Modifier,
    busy: Boolean = false,
    error: String? = null,
) {
    if (editing) {
        MemeDockInlineEdit("", placeholder, onCreate, onCancel,
            modifier.padding(horizontal = MemeDockLayout.PagePadding, vertical = 4.dp), busy = busy, error = error)
    } else {
        Row(
            modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight)
                .clickable(role = Role.Button, onClick = onStart).padding(horizontal = MemeDockLayout.PagePadding),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Icon(MemeDockIcons.Add, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.primary)
            Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.primary)
        }
    }
}
