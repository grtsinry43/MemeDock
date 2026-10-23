package com.grtsinry43.memedock.feature.sync

import android.text.format.DateUtils
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import dev.chrisbanes.haze.rememberHazeState

@Composable
fun SyncScreen(state: SyncState, back: () -> Unit, connect: (String, String) -> Unit, now: () -> Unit,
    pause: (Boolean) -> Unit, disconnect: () -> Unit, wifiOnly: (Boolean) -> Unit) {
    var origin by remember(state.binding, state.status.origin) { mutableStateOf(state.status.origin.orEmpty()) }
    // Never save tokens in navigation/saved instance state.
    var token by remember { mutableStateOf("") }
    var reauthorizing by remember(state.binding) { mutableStateOf(false) }
    var disconnecting by remember { mutableStateOf(false) }
    val authorizing = reauthorizing || !state.hasCredential || state.status.error.equals("unauthorized", true)
    val scroll = rememberScrollState()
    val focus = LocalFocusManager.current
    val keyboard = LocalSoftwareKeyboardController.current
    val glass = rememberHazeState()
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        Column(Modifier.fillMaxSize().glassSource(glass).verticalScroll(scroll)
            .padding(top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + MemeDockLayout.TopBarHeight,
                bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding() + 24.dp)) {
            MemeDockPageHeader(stringResource(R.string.sync_title))
            Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.sync_token_hint), style = MaterialTheme.typography.bodySmall)
                if (authorizing) {
                    MemeDockTextField(origin, { origin = it }, stringResource(R.string.sync_server), Modifier.testTag("sync-server"),
                        enabled = !state.busy, placeholder = "https://", keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri))
                    MemeDockTextField(token, { token = it }, stringResource(R.string.sync_token), Modifier.testTag("sync-token"),
                        enabled = !state.busy, visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password))
                    MemeDockButton(stringResource(R.string.sync_connect), {
                        val credential = token; token = ""
                        focus.clearFocus(); keyboard?.hide()
                        connect(origin, credential)
                    }, Modifier.testTag("sync-connect"), enabled = !state.busy && state.binding != null && origin.isNotBlank() && token.isNotEmpty())
                } else {
                    Text(state.status.origin.orEmpty(), style = MaterialTheme.typography.bodyMedium)
                    Text(state.status.deviceId.orEmpty(), style = MaterialTheme.typography.bodySmall)
                }
            }
            if (state.status.connected) MemeDockGroup(title = stringResource(R.string.sync_status)) {
                row { MemeDockRow(stringResource(R.string.sync_status), value = stringResource(when {
                    state.busy -> R.string.sync_running
                    !state.hasCredential || state.status.error.equals("unauthorized", true) -> R.string.sync_authorize
                    state.error != null || state.status.error != null -> R.string.sync_failed
                    state.status.paused -> R.string.sync_paused
                    else -> R.string.sync_ready
                })) }
                row { MemeDockRow(stringResource(R.string.sync_pending), value = state.status.pending.toString()) }
                row { MemeDockRow(stringResource(R.string.sync_blocked), value = state.status.blocked.toString()) }
                row { MemeDockRow(stringResource(R.string.sync_last_success), value = state.status.lastSuccessMs?.let {
                    DateUtils.getRelativeTimeSpanString(it).toString()
                } ?: stringResource(R.string.sync_never)) }
            }
            Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(stringResource(R.string.sync_wifi_only), Modifier.weight(1f))
                    Switch(state.wifiOnly, wifiOnly, Modifier.testTag("sync-wifi-only"), enabled = !state.busy)
                }
                if (state.hasCredential && !authorizing) {
                    MemeDockButton(stringResource(if (state.status.paused) R.string.sync_resume else R.string.sync_now),
                        { if (state.status.paused) pause(false) else now() }, Modifier.testTag("sync-now"), enabled = !state.busy)
                    if (!state.status.paused) MemeDockTextButton(stringResource(R.string.sync_pause), { pause(true) }, enabled = !state.busy)
                    MemeDockTextButton(stringResource(R.string.sync_reauthorize), { pause(true); reauthorizing = true }, enabled = !state.busy)
                }
                if (state.status.connected && state.hasCredential) MemeDockTextButton(stringResource(R.string.sync_disconnect),
                    { disconnecting = true }, enabled = !state.busy, destructive = true)
                val error = state.error ?: state.status.error
                if (error != null) Text(failureText(error), color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("sync-error"))
                if (state.status.blocked > 0) Text(stringResource(R.string.sync_blocked_hint), style = MaterialTheme.typography.bodySmall)
                Text(stringResource(R.string.sync_background_hint), style = MaterialTheme.typography.bodySmall)
                Text(stringResource(R.string.sync_merge_hint), style = MaterialTheme.typography.bodySmall)
            }
        }
        MemeDockTopBar("", glass, back = back, scrolled = scroll.canScrollBackward)
    }
    LaunchedEffect(state.hasCredential, state.status.paused, state.error) {
        if (state.hasCredential && !state.status.paused && state.error == null) reauthorizing = false
    }
    if (disconnecting) AlertDialog(onDismissRequest = { disconnecting = false },
        title = { Text(stringResource(R.string.sync_disconnect)) }, text = { Text(stringResource(R.string.sync_disconnect_hint)) },
        confirmButton = { TextButton({ disconnecting = false; disconnect() }) { Text(stringResource(R.string.sync_disconnect)) } },
        dismissButton = { TextButton({ disconnecting = false }) { Text(stringResource(R.string.cancel)) } })
}
