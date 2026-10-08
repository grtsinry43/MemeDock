package com.grtsinry43.memedock.feature.trash

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureCode
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import kotlinx.coroutines.CancellationException

/**
 * Confirms restoring sticker [id] and lists where it used to be, since restoring does not put it back there.
 * The caller turns [visible] off once [restore] succeeds; a failure stays in the sheet as [error].
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun RestoreSheet(visible: Boolean, id: String, repository: ManagementRepository, busy: Boolean, error: String?,
    dismiss: () -> Unit, restore: () -> Unit) {
    var suggestions by remember(id) { mutableStateOf<RestoreSuggestions?>(null) }
    var loadError by remember(id) { mutableStateOf<String?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    LaunchedEffect(id, visible, refresh) {
        if (!visible) return@LaunchedEffect
        loadError = null
        try { suggestions = repository.suggestions(id) }
        catch (cancel: CancellationException) { throw cancel }
        catch (failure: Exception) { loadError = failureCode(failure) }
    }
    MemeDockSheet(visible, dismiss, title = stringResource(R.string.restore_sticker_title),
        subtitle = stringResource(R.string.restore_previous_hint), dismissible = !busy) {
        val history = suggestions
        when {
            history == null && loadError == null -> MemeDockSkeleton(
                Modifier.padding(horizontal = 24.dp, vertical = 8.dp).fillMaxWidth(.6f).height(20.dp), MaterialTheme.shapes.extraSmall)
            history != null && (history.collection != null || history.tags.isNotEmpty()) -> {
                MemeDockSectionHeader(stringResource(R.string.restore_previous), Modifier.padding(top = MemeDockLayout.GapSmall),
                    inset = 24.dp)
                FlowRow(Modifier.fillMaxWidth().padding(horizontal = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    history.collection?.let { MemeDockChip(it.name, icon = MemeDockIcons.Folder) }
                    history.tags.forEach { MemeDockChip(it.name, icon = MemeDockIcons.Label) }
                }
            }
        }
        (error ?: loadError)?.let { code ->
            Row(Modifier.fillMaxWidth().padding(start = 24.dp, end = 12.dp, top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(failureText(code), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error)
                if (loadError != null) TextButton(onClick = { refresh++ }, enabled = !busy) { Text(stringResource(R.string.retry)) }
            }
        }
        MemeDockButton(stringResource(R.string.restore), restore,
            Modifier.padding(horizontal = 24.dp).padding(top = 20.dp).testTag("confirm-restore-sticker"), busy = busy)
    }
}
