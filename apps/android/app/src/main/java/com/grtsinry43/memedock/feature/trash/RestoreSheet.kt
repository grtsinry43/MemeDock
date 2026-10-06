package com.grtsinry43.memedock.feature.trash

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureCode
import com.grtsinry43.memedock.ui.failureText
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
            history != null && (history.collections.isNotEmpty() || history.tags.isNotEmpty()) -> {
                Text(stringResource(R.string.restore_previous), style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
                FlowRow(Modifier.fillMaxWidth().padding(horizontal = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    history.collections.forEach { PlaceChip(it.name, MemeDockIcons.Folder) }
                    history.tags.forEach { PlaceChip(it.name, MemeDockIcons.Label) }
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
        Button(
            onClick = { if (!busy) restore() },
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(top = 20.dp).height(52.dp)
                .testTag("confirm-restore-sticker"),
            shape = MaterialTheme.shapes.medium,
        ) {
            if (busy) CircularProgressIndicator(Modifier.size(20.dp), color = MaterialTheme.colorScheme.onPrimary, strokeWidth = 2.dp)
            else Text(stringResource(R.string.restore))
        }
    }
}

@Composable
private fun PlaceChip(name: String, icon: ImageVector) {
    val colors = MaterialTheme.colorScheme
    Row(Modifier.heightIn(min = 32.dp).background(colors.surfaceContainer, MaterialTheme.shapes.small).padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Icon(icon, null, Modifier.size(16.dp), tint = colors.onSurfaceVariant)
        Text(name, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}
