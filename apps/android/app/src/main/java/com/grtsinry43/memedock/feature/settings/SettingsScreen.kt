package com.grtsinry43.memedock.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.settings.LanguageMode
import com.grtsinry43.memedock.data.settings.ThemeMode
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout

private enum class SettingsSheet { Theme, Language }

@Composable
fun SettingsScreen(state: SettingsState, version: String, select: (ThemeMode) -> Unit, selectLanguage: (LanguageMode) -> Unit,
    retry: () -> Unit, trash: () -> Unit, backup: () -> Unit, licenses: () -> Unit, contentPadding: PaddingValues) {
    var sheet by remember { mutableStateOf<SettingsSheet?>(null) }
    Column(
        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).verticalScroll(rememberScrollState())
            .windowInsetsPadding(WindowInsets.statusBars)
            .padding(bottom = contentPadding.calculateBottomPadding() + MemeDockLayout.SectionGap),
    ) {
        MemeDockPageHeader(stringResource(R.string.tab_mine))
        Spacer(Modifier.height(MemeDockLayout.GapMedium))
        Overview(state, retry)
        Spacer(Modifier.height(MemeDockLayout.SectionGap))
        MemeDockGroup(title = stringResource(R.string.settings_library)) {
            row { MemeDockRow(stringResource(R.string.trash), Modifier.testTag("settings-trash"), icon = MemeDockIcons.Delete, onClick = trash) }
            row {
                MemeDockRow(stringResource(R.string.backup_title), Modifier.testTag("settings-backup"), icon = MemeDockIcons.BackupRestore,
                    onClick = backup)
            }
        }
        Spacer(Modifier.height(MemeDockLayout.SectionGap))
        val generalError = listOfNotNull(
            stringResource(R.string.settings_save_failed).takeIf { state.appearanceError },
            stringResource(R.string.settings_language_save_failed).takeIf { state.languageError },
        ).joinToString("\n").ifEmpty { null }
        MemeDockGroup(title = stringResource(R.string.settings_general), footer = generalError) {
            row {
                MemeDockRow(stringResource(R.string.settings_appearance), Modifier.testTag("settings-theme"), icon = MemeDockIcons.Contrast,
                    value = state.mode?.let { stringResource(it.label) }, enabled = state.mode != null && !state.saving,
                    onClick = { sheet = SettingsSheet.Theme })
            }
            row {
                MemeDockRow(stringResource(R.string.settings_language), Modifier.testTag("settings-language"), icon = MemeDockIcons.Language,
                    value = state.languageMode?.let { stringResource(it.label) }, enabled = state.languageMode != null && !state.savingLanguage,
                    onClick = { sheet = SettingsSheet.Language })
            }
        }
        Spacer(Modifier.height(MemeDockLayout.SectionGap))
        MemeDockGroup(title = stringResource(R.string.settings_about)) {
            row { MemeDockRow(stringResource(R.string.settings_version), icon = MemeDockIcons.Info, value = version) }
            row {
                MemeDockRow(stringResource(R.string.settings_licenses), Modifier.testTag("settings-licenses"), icon = MemeDockIcons.Description,
                    onClick = licenses)
            }
        }
    }
    MemeDockSheet(sheet == SettingsSheet.Theme, { sheet = null }, title = stringResource(R.string.settings_appearance)) {
        ThemeMode.entries.forEach { mode ->
            MemeDockSheetChoice(stringResource(mode.label), state.mode == mode, { select(mode); dismiss() },
                Modifier.testTag("theme:${mode.name}"), enabled = !state.saving)
        }
    }
    MemeDockSheet(sheet == SettingsSheet.Language, { sheet = null }, title = stringResource(R.string.settings_language)) {
        LanguageMode.entries.forEach { mode ->
            MemeDockSheetChoice(stringResource(mode.label), state.languageMode == mode, { dismiss(); selectLanguage(mode) },
                Modifier.testTag("language:${mode.name}"), enabled = !state.savingLanguage)
        }
    }
}

@Composable
private fun Overview(state: SettingsState, retry: () -> Unit) {
    val statistics = state.statistics
    Column(Modifier.fillMaxWidth().padding(horizontal = MemeDockLayout.PagePadding)) {
        Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLowest) {
            if (state.error != null) Row(Modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight).padding(start = 16.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically) {
                Text(failureText(state.error), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error)
                TextButton(onClick = retry) { Text(stringResource(R.string.retry)) }
            } else Row(Modifier.fillMaxWidth().padding(vertical = 16.dp)) {
                Statistic(stringResource(R.string.settings_original_count), statistics?.originalCount?.toString(), Modifier.weight(1f))
                Statistic(stringResource(R.string.settings_saved_originals), statistics?.let { formatFileSize(it.savedOriginalBytes) },
                    Modifier.weight(1f))
                Statistic(stringResource(R.string.settings_cache_usage), statistics?.let { formatFileSize(it.cachedBytes) },
                    Modifier.weight(1f).testTag("settings-cache-usage"))
            }
        }
        Text(stringResource(R.string.settings_cache_footer), style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp))
    }
}

@Composable
private fun Statistic(label: String, value: String?, modifier: Modifier = Modifier) {
    Column(modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.heightIn(min = 28.dp), contentAlignment = Alignment.CenterStart) {
            if (value == null) MemeDockSkeleton(Modifier.size(width = 56.dp, height = 20.dp), MaterialTheme.shapes.extraSmall)
            else Text(value, style = MaterialTheme.typography.titleLarge, maxLines = 1)
        }
        Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
    }
}

private val ThemeMode.label get() = when (this) {
    ThemeMode.System -> R.string.theme_system
    ThemeMode.Light -> R.string.theme_light
    ThemeMode.Dark -> R.string.theme_dark
}

private val LanguageMode.label get() = when (this) {
    LanguageMode.System -> R.string.language_system
    LanguageMode.Chinese -> R.string.language_chinese
    LanguageMode.English -> R.string.language_english
}
