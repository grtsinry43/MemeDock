package com.grtsinry43.memedock.feature.settings

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.settings.ThemeMode
import com.grtsinry43.memedock.ui.components.formatFileSize
import com.grtsinry43.memedock.ui.failureText

@Composable
fun SettingsScreen(state: SettingsState, select: (ThemeMode) -> Unit, retry: () -> Unit,
    tags: (() -> Unit)? = null, trash: (() -> Unit)? = null, backup: (() -> Unit)? = null) {
    Column(Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text(stringResource(R.string.tab_settings), style = MaterialTheme.typography.headlineLarge)
        tags?.let { action -> OutlinedButton(onClick = action, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.manage_tags)) } }
        trash?.let { action -> OutlinedButton(onClick = action, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.trash)) } }
        backup?.let { action -> OutlinedButton(onClick = action, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.backup_title)) } }
        Text(stringResource(R.string.settings_appearance), style = MaterialTheme.typography.titleMedium)
        Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow) {
            Column(Modifier.fillMaxWidth().selectableGroup()) {
                ThemeMode.entries.forEach { mode ->
                    Row(Modifier.fillMaxWidth().selectable(selected = state.mode == mode, enabled = !state.saving,
                        role = Role.RadioButton, onClick = { select(mode) }).padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically) {
                        Text(stringResource(when (mode) {
                            ThemeMode.System -> R.string.theme_system
                            ThemeMode.Light -> R.string.theme_light
                            ThemeMode.Dark -> R.string.theme_dark
                        }), modifier = Modifier.weight(1f))
                        RadioButton(selected = state.mode == mode, onClick = null)
                    }
                }
            }
        }
        if (state.appearanceError) Text(stringResource(R.string.settings_save_failed), color = MaterialTheme.colorScheme.error)
        Text(stringResource(R.string.settings_storage), style = MaterialTheme.typography.titleMedium)
        Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow) {
            Column(Modifier.fillMaxWidth().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                when {
                    state.loading -> CircularProgressIndicator(Modifier.size(24.dp))
                    state.error != null -> {
                        Text(failureText(state.error))
                        TextButton(onClick = retry) { Text(stringResource(R.string.retry)) }
                    }
                    state.statistics != null -> {
                        Row(Modifier.fillMaxWidth()) {
                            Text(stringResource(R.string.settings_original_count), Modifier.weight(1f))
                            Text(state.statistics.originalCount.toString())
                        }
                        Row(Modifier.fillMaxWidth()) {
                            Text(stringResource(R.string.settings_saved_originals), Modifier.weight(1f))
                            Text(formatFileSize(state.statistics.savedOriginalBytes))
                        }
                    }
                }
            }
        }
    }
}
