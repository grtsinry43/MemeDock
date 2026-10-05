package com.grtsinry43.memedock.feature.trash

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.failureText
import kotlinx.coroutines.CancellationException

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RestoreSheet(id: String, repository: ManagementRepository, busy: Boolean, error: String?, dismiss: () -> Unit, restore: () -> Unit) {
    var suggestions by remember(id) { mutableStateOf<RestoreSuggestions?>(null) }
    var loadError by remember { mutableStateOf<String?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    LaunchedEffect(id, refresh) {
        try { suggestions = repository.suggestions(id); loadError = null }
        catch (cancel: CancellationException) { throw cancel }
        catch (failure: Exception) { loadError = (failure as? LibraryFailure)?.reason ?: "INTERNAL" }
    }
    ModalBottomSheet(onDismissRequest = { if (!busy) dismiss() }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(stringResource(R.string.restore), style = MaterialTheme.typography.headlineSmall)
            Text(stringResource(R.string.restore_previous_hint), color = MaterialTheme.colorScheme.onSurfaceVariant)
            suggestions?.let { history ->
                if (history.collections.isNotEmpty() || history.tags.isNotEmpty()) {
                    Text(stringResource(R.string.restore_previous), style = MaterialTheme.typography.titleMedium)
                    history.collections.forEach { Text(stringResource(R.string.detail_collections_title) + " · " + it.name) }
                    history.tags.forEach { Text(stringResource(R.string.detail_tags_title) + " · " + it.name) }
                }
            }
            if (suggestions == null && loadError == null) CircularProgressIndicator(Modifier.size(24.dp))
            (error ?: loadError)?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error) }
            if (loadError != null) TextButton(onClick = { refresh++ }, enabled = !busy) { Text(stringResource(R.string.retry)) }
            Button(onClick = restore, enabled = !busy, modifier = Modifier.fillMaxWidth().testTag("confirm-restore-sticker")) { Text(stringResource(R.string.restore)) }
        }
    }
}
