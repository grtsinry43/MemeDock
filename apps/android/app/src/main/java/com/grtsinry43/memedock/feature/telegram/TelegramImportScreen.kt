package com.grtsinry43.memedock.feature.telegram

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.telegram.*
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import dev.chrisbanes.haze.rememberHazeState
import kotlinx.coroutines.CancellationException
import java.io.File

@Composable
fun TelegramImportScreen(state: TelegramImportState, model: TelegramImportViewModel, imageLoader: ImageLoader, back: () -> Unit) {
    val glass = rememberHazeState()
    val grid = rememberLazyGridState()
    val keyboard = LocalSoftwareKeyboardController.current
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + MemeDockLayout.TopBarHeight
    val bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val pack = state.pack
    val results = remember(state.report) { state.report?.items?.associateBy { it.id }.orEmpty() }
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)
        .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))) {
        LazyVerticalGrid(columns = GridCells.Adaptive(104.dp), state = grid,
            modifier = Modifier.fillMaxSize().glassSource(glass).testTag("telegram-grid"),
            contentPadding = PaddingValues(top = top + 8.dp, bottom = bottom + if (pack == null) 24.dp else 100.dp)) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Column(Modifier.padding(horizontal = MemeDockLayout.PagePadding, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(pack?.title ?: stringResource(R.string.telegram_title), style = MaterialTheme.typography.headlineLarge)
                    if (pack == null) {
                        TokenInputs(state, model)
                        OutlinedTextField(state.name, model::name, Modifier.fillMaxWidth().testTag("telegram-pack"),
                            enabled = !state.busy, singleLine = true, label = { Text(stringResource(R.string.telegram_pack)) },
                            placeholder = { Text(stringResource(R.string.telegram_pack_placeholder)) }, shape = MaterialTheme.shapes.medium)
                        if (state.loading) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                                Text(stringResource(R.string.telegram_loading), Modifier.weight(1f).padding(start = 12.dp))
                                TextButton(onClick = model::cancel) { Text(stringResource(R.string.cancel)) }
                            }
                        } else Button(onClick = { keyboard?.hide(); model.load() },
                            enabled = !state.busy && state.token.isNotBlank() && state.name.isNotBlank(),
                            modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).testTag("telegram-load"), shape = MaterialTheme.shapes.medium) {
                            Text(stringResource(R.string.telegram_load))
                        }
                    } else {
                        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                            Text(stringResource(R.string.telegram_choose), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant)
                            TextButton(onClick = model::changePack, enabled = !state.busy) { Text(stringResource(R.string.telegram_change_pack)) }
                        }
                        Row {
                            TextButton(onClick = model::selectAll, enabled = !state.busy) { Text(stringResource(R.string.telegram_select_new)) }
                            TextButton(onClick = model::clearSelection, enabled = !state.busy && state.selected.isNotEmpty()) { Text(stringResource(R.string.clear)) }
                        }
                        if (!state.importing) state.report?.let { report -> ImportSummary(report) }
                    }
                    if (state.credentialError) Text(stringResource(R.string.telegram_token_storage_error), color = MaterialTheme.colorScheme.error)
                    state.error?.let { Text(telegramFailure(it), color = MaterialTheme.colorScheme.error) }
                }
            }
            if (pack != null) items(pack.items, key = { it.id }) { item ->
                StickerChoice(pack, item, state.itemStates[item.id] ?: item.state, item.id in state.selected,
                    !state.busy, results[item.id], model, imageLoader)
            }
        }
        val scrolled by remember { derivedStateOf { grid.firstVisibleItemIndex > 0 || grid.firstVisibleItemScrollOffset > 0 } }
        MemeDockTopBar(stringResource(R.string.telegram_title), glass, back = back, scrolled = scrolled)
        if (pack != null) Surface(Modifier.align(Alignment.BottomCenter).navigationBarsPadding()
            .padding(horizontal = 12.dp, vertical = 8.dp).fillMaxWidth(), shape = MaterialTheme.shapes.medium,
            color = MaterialTheme.colorScheme.surfaceContainerHigh, tonalElevation = 0.dp, shadowElevation = 1.dp) {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp).heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(if (state.importing) stringResource(R.string.telegram_progress, state.report?.completed ?: 0,
                        state.report?.items?.size ?: 0) else stringResource(R.string.selection_count, state.selected.size),
                        style = MaterialTheme.typography.labelLarge)
                    if (state.importing) LinearProgressIndicator(progress = {
                        val total = state.report?.items?.size ?: 0
                        if (total == 0) 0f else (state.report?.completed ?: 0).toFloat() / total
                    }, modifier = Modifier.padding(top = 4.dp).fillMaxWidth())
                }
                Spacer(Modifier.width(12.dp))
                if (state.importing) TextButton(onClick = model::cancel, enabled = !state.stopping) {
                    Text(stringResource(if (state.stopping) R.string.telegram_stopping else R.string.cancel))
                } else TextButton(onClick = model::importSelected, enabled = state.selected.isNotEmpty() && !state.busy,
                    modifier = Modifier.testTag("telegram-import")) { Text(stringResource(R.string.telegram_import)) }
            }
        }
    }
}

@Composable
private fun TokenInputs(state: TelegramImportState, model: TelegramImportViewModel) {
    var revealed by remember { mutableStateOf(false) }
    OutlinedTextField(state.token, model::token, Modifier.fillMaxWidth().testTag("telegram-token"),
        enabled = !state.busy, singleLine = true, label = { Text(stringResource(R.string.telegram_token)) },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
        visualTransformation = if (revealed) VisualTransformation.None else PasswordVisualTransformation(),
        trailingIcon = { TextButton(onClick = { revealed = !revealed }) {
            Text(stringResource(if (revealed) R.string.telegram_hide else R.string.telegram_show))
        } }, shape = MaterialTheme.shapes.medium)
    Text(stringResource(R.string.telegram_token_hint), style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant)
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Row(Modifier.weight(1f).toggleable(state.rememberToken, enabled = !state.busy, role = Role.Checkbox,
            onValueChange = model::remember), verticalAlignment = Alignment.CenterVertically) {
            Checkbox(state.rememberToken, null, enabled = !state.busy)
            Text(stringResource(R.string.telegram_remember), style = MaterialTheme.typography.bodyMedium)
        }
        TextButton(onClick = model::forget, enabled = !state.busy) { Text(stringResource(R.string.telegram_forget)) }
    }
}

private data class Preview(val path: String? = null, val failed: Boolean = false)

@Composable
private fun StickerChoice(pack: TelegramPack, item: TelegramItem, status: TelegramItemState, selected: Boolean,
    enabled: Boolean, result: TelegramResult?, model: TelegramImportViewModel, imageLoader: ImageLoader) {
    var attempt by remember(pack, item.id) { mutableIntStateOf(0) }
    val preview by produceState(Preview(), pack, item.id, attempt) {
        value = Preview()
        if (!item.hasPreview) return@produceState
        try { value = Preview(model.preview(pack, item.id)) }
        catch (cancel: CancellationException) { throw cancel }
        catch (_: Exception) { value = Preview(failed = true) }
    }
    val context = LocalContext.current
    val description = stringResource(R.string.telegram_sticker, item.emoji.orEmpty())
    val label = when {
        result?.outcome == TelegramOutcome.Failed -> stringResource(R.string.telegram_failed)
        status == TelegramItemState.Imported -> stringResource(R.string.telegram_imported)
        status == TelegramItemState.RestoreRequired -> stringResource(R.string.telegram_restore_required)
        status == TelegramItemState.OriginalMissing -> stringResource(R.string.telegram_redownload)
        item.animated -> stringResource(R.string.telegram_animated)
        else -> null
    }
    Column(Modifier.padding(2.dp).clip(MaterialTheme.shapes.small)
        .background(if (selected) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerLow)
        .toggleable(selected, enabled = enabled && status != TelegramItemState.RestoreRequired, role = Role.Checkbox,
            onValueChange = { model.toggle(item.id) }).semantics { contentDescription = listOfNotNull(description, label).joinToString(", ") }) {
        Box(Modifier.fillMaxWidth().aspectRatio(1f), contentAlignment = Alignment.Center) {
            when {
                preview.path != null -> {
                    val request = remember(preview.path, context) { ImageRequest.Builder(context).data(File(preview.path!!)).size(256, 256).build() }
                    AsyncImage(request, description, imageLoader, Modifier.fillMaxSize().padding(8.dp))
                }
                preview.failed -> TextButton(onClick = { attempt++ }, enabled = enabled) { Text(stringResource(R.string.retry)) }
                !item.hasPreview -> Text(item.emoji.orEmpty().ifBlank { stringResource(R.string.telegram_no_preview) },
                    style = MaterialTheme.typography.bodySmall)
                else -> CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            }
            if (selected) Icon(MemeDockIcons.Check, null, Modifier.align(Alignment.TopEnd).padding(6.dp).size(20.dp),
                tint = MaterialTheme.colorScheme.primary)
        }
        if (label != null) Text(label, Modifier.padding(horizontal = 6.dp, vertical = 4.dp),
            style = MaterialTheme.typography.labelSmall, maxLines = 1, overflow = TextOverflow.Ellipsis,
            color = if (result?.outcome == TelegramOutcome.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun ImportSummary(report: TelegramReport) {
    val added = report.items.count { it.outcome == TelegramOutcome.Created }
    val existing = report.items.count { it.outcome == TelegramOutcome.Reused }
    val failed = report.items.count { it.outcome == TelegramOutcome.Failed }
    val restore = report.items.count { it.outcome == TelegramOutcome.RestoreRequired }
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(stringResource(if (report.stopped) R.string.telegram_stopped else R.string.telegram_finished), style = MaterialTheme.typography.titleMedium)
        Text(stringResource(R.string.telegram_summary, added, existing, failed), style = MaterialTheme.typography.bodyMedium)
        if (restore > 0) Text(stringResource(R.string.telegram_restore_hint), style = MaterialTheme.typography.bodySmall)
        report.items.firstOrNull { it.error != null }?.error?.let { Text(telegramFailure(it), color = MaterialTheme.colorScheme.error) }
    }
}

@Composable
private fun telegramFailure(code: String) = when (code) {
    "UNAUTHORIZED" -> stringResource(R.string.telegram_invalid_token)
    "NOT_FOUND" -> stringResource(R.string.telegram_pack_missing)
    "NETWORK", "TIMEOUT" -> stringResource(R.string.telegram_network_error)
    "RATE_LIMITED" -> stringResource(R.string.telegram_rate_limit)
    else -> failureText(code)
}
