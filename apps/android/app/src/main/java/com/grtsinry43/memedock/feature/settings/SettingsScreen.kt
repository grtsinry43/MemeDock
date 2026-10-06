package com.grtsinry43.memedock.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.settings.LanguageMode
import com.grtsinry43.memedock.data.settings.ThemeMode
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout

@Composable
fun SettingsScreen(state: SettingsState, select: (ThemeMode) -> Unit, selectLanguage: (LanguageMode) -> Unit,
    retry: () -> Unit, trash: () -> Unit, backup: () -> Unit, contentPadding: PaddingValues) {
    Column(
        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).verticalScroll(rememberScrollState())
            .windowInsetsPadding(WindowInsets.statusBars)
            .padding(bottom = contentPadding.calculateBottomPadding() + MemeDockLayout.SectionGap),
    ) {
        Text(stringResource(R.string.tab_mine), style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.padding(horizontal = MemeDockLayout.PagePadding).padding(top = 18.dp, bottom = 18.dp)
                .semantics { heading() })
        val statistics = state.statistics
        MemeDockGroup(title = stringResource(R.string.settings_library),
            footer = state.error?.let { failureText(it) }) {
            row {
                MemeDockRow(stringResource(R.string.settings_original_count), value = statistics?.originalCount?.toString()) {
                    when {
                        state.error != null -> TextButton(onClick = retry) { Text(stringResource(R.string.retry)) }
                        statistics == null -> MemeDockSkeleton(Modifier.size(width = 40.dp, height = 16.dp), MaterialTheme.shapes.extraSmall)
                    }
                }
            }
            row {
                MemeDockRow(stringResource(R.string.settings_saved_originals),
                    value = statistics?.let { formatFileSize(it.savedOriginalBytes) }) {
                    if (statistics == null && state.error == null)
                        MemeDockSkeleton(Modifier.size(width = 56.dp, height = 16.dp), MaterialTheme.shapes.extraSmall)
                }
            }
            row {
                MemeDockRow(stringResource(R.string.settings_cache_usage), Modifier.testTag("settings-cache-usage"),
                    subtitle = stringResource(R.string.settings_cache_usage_hint),
                    value = statistics?.let { formatFileSize(it.cachedBytes) }) {
                    if (statistics == null && state.error == null)
                        MemeDockSkeleton(Modifier.size(width = 56.dp, height = 16.dp), MaterialTheme.shapes.extraSmall)
                }
            }
            row { MemeDockRow(stringResource(R.string.trash), Modifier.testTag("settings-trash"), icon = MemeDockIcons.Delete, onClick = trash) }
            row {
                MemeDockRow(stringResource(R.string.backup_title), Modifier.testTag("settings-backup"), icon = MemeDockIcons.BackupRestore,
                    onClick = backup)
            }
        }
        Spacer(Modifier.height(MemeDockLayout.SectionGap))
        MemeDockGroup(title = stringResource(R.string.settings_appearance),
            footer = if (state.appearanceError) stringResource(R.string.settings_save_failed) else null) {
            ThemeMode.entries.forEach { mode ->
                row { ThemeRow(mode, state.mode == mode, !state.saving) { select(mode) } }
            }
        }
        Spacer(Modifier.height(MemeDockLayout.SectionGap))
        MemeDockGroup(title = stringResource(R.string.settings_language),
            footer = if (state.languageError) stringResource(R.string.settings_language_save_failed) else null) {
            LanguageMode.entries.forEach { mode ->
                row { LanguageRow(mode, state.languageMode == mode, !state.savingLanguage) { selectLanguage(mode) } }
            }
        }
    }
}

@Composable
private fun ThemeRow(mode: ThemeMode, selected: Boolean, enabled: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight)
            .selectable(selected, enabled = enabled, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 16.dp).testTag("theme:${mode.name}"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(stringResource(when (mode) {
            ThemeMode.System -> R.string.theme_system
            ThemeMode.Light -> R.string.theme_light
            ThemeMode.Dark -> R.string.theme_dark
        }), Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge)
        if (selected) Icon(MemeDockIcons.Check, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.primary)
    }
}

@Composable
private fun LanguageRow(mode: LanguageMode, selected: Boolean, enabled: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight)
            .selectable(selected, enabled = enabled, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 16.dp).testTag("language:${mode.name}"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(stringResource(when (mode) {
            LanguageMode.System -> R.string.language_system
            LanguageMode.Chinese -> R.string.language_chinese
            LanguageMode.English -> R.string.language_english
        }), Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge)
        if (selected) Icon(MemeDockIcons.Check, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.primary)
    }
}
