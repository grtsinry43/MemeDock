package com.grtsinry43.memedock.feature.telegram

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
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
import com.grtsinry43.memedock.ui.theme.MemeDockMotion
import dev.chrisbanes.haze.rememberHazeState
import kotlinx.coroutines.CancellationException
import java.io.File

private val GridGutter = 12.dp

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
            contentPadding = PaddingValues(start = GridGutter, end = GridGutter, top = top,
                bottom = bottom + if (pack == null) MemeDockLayout.SectionGap else 88.dp),
            horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.GapXSmall),
            verticalArrangement = Arrangement.spacedBy(MemeDockLayout.GapXSmall)) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Column(Modifier.padding(horizontal = MemeDockLayout.PagePadding - GridGutter).padding(top = MemeDockLayout.GapSmall,
                    bottom = MemeDockLayout.GapSmall), verticalArrangement = Arrangement.spacedBy(MemeDockLayout.GapMedium)) {
                    if (pack == null) {
                        TokenInputs(state, model)
                        MemeDockTextField(state.name, model::name, stringResource(R.string.telegram_pack), Modifier.testTag("telegram-pack"),
                            enabled = !state.busy, placeholder = stringResource(R.string.telegram_pack_placeholder))
                        if (state.loading) {
                            Row(Modifier.heightIn(min = MemeDockLayout.ButtonHeight), verticalAlignment = Alignment.CenterVertically) {
                                CircularProgressIndicator(Modifier.size(MemeDockLayout.IconMedium), strokeWidth = 2.dp)
                                Text(stringResource(R.string.telegram_loading), Modifier.weight(1f).padding(start = MemeDockLayout.GapMedium))
                                TextButton(onClick = model::cancel) { Text(stringResource(R.string.cancel)) }
                            }
                        } else MemeDockButton(stringResource(R.string.telegram_load), { keyboard?.hide(); model.load() },
                            Modifier.testTag("telegram-load"),
                            enabled = !state.busy && state.token.isNotBlank() && state.name.isNotBlank())
                    } else {
                        Column {
                            MemeDockSectionHeader(stringResource(R.string.telegram_choose), inset = 0.dp,
                                action = stringResource(R.string.telegram_change_pack), actionEnabled = !state.busy,
                                onAction = model::changePack)
                            Row(horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.GapSmall)) {
                                MemeDockChip(stringResource(R.string.telegram_select_new), enabled = !state.busy, onClick = model::selectAll)
                                MemeDockChip(stringResource(R.string.clear), enabled = !state.busy && state.selected.isNotEmpty(),
                                    onClick = model::clearSelection)
                            }
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
        MemeDockTopBar(pack?.title ?: stringResource(R.string.telegram_title), glass, back = back, scrolled = scrolled)
        if (pack != null) MemeDockFloatingBar(glass, Modifier.align(Alignment.BottomCenter).navigationBarsPadding()
            .padding(horizontal = MemeDockLayout.PagePadding).padding(bottom = MemeDockLayout.GapMedium)) {
            Column(Modifier.weight(1f).padding(vertical = MemeDockLayout.GapSmall)) {
                Text(if (state.importing) stringResource(R.string.telegram_progress, state.report?.completed ?: 0,
                    state.report?.items?.size ?: 0) else stringResource(R.string.selection_count, state.selected.size),
                    style = MaterialTheme.typography.bodyMedium)
                if (state.importing) LinearProgressIndicator(progress = {
                    val total = state.report?.items?.size ?: 0
                    if (total == 0) 0f else (state.report?.completed ?: 0).toFloat() / total
                }, modifier = Modifier.padding(top = MemeDockLayout.GapXSmall).fillMaxWidth())
            }
            if (state.importing) TextButton(onClick = model::cancel, enabled = !state.stopping) {
                Text(stringResource(if (state.stopping) R.string.telegram_stopping else R.string.cancel))
            } else TextButton(onClick = model::importSelected, enabled = state.selected.isNotEmpty() && !state.busy,
                modifier = Modifier.testTag("telegram-import")) { Text(stringResource(R.string.telegram_import)) }
        }
    }
}

@Composable
private fun TokenInputs(state: TelegramImportState, model: TelegramImportViewModel) {
    var revealed by remember { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(MemeDockLayout.GapSmall)) {
        MemeDockTextField(state.token, model::token, stringResource(R.string.telegram_token), Modifier.testTag("telegram-token"),
            enabled = !state.busy,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
            visualTransformation = if (revealed) VisualTransformation.None else PasswordVisualTransformation(),
            trailing = { TextButton(onClick = { revealed = !revealed }) {
                Text(stringResource(if (revealed) R.string.telegram_hide else R.string.telegram_show))
            } })
        Text(stringResource(R.string.telegram_token_hint), Modifier.padding(horizontal = MemeDockLayout.GapLarge),
            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.weight(1f).clip(MaterialTheme.shapes.small)
                .toggleable(state.rememberToken, enabled = !state.busy, role = Role.Checkbox, onValueChange = model::remember)
                .heightIn(min = 48.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.GapMedium)) {
                Checkbox(state.rememberToken, null, enabled = !state.busy)
                Text(stringResource(R.string.telegram_remember), style = MaterialTheme.typography.bodyMedium)
            }
            TextButton(onClick = model::forget, enabled = !state.busy) { Text(stringResource(R.string.telegram_forget)) }
        }
    }
}

private data class Preview(val path: String? = null, val failed: Boolean = false)

/** Follows the library tile: bare artwork, a rounded highlight and a shrink when selected, status as a corner label. */
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
    val colors = MaterialTheme.colorScheme
    val description = stringResource(R.string.telegram_sticker, item.emoji.orEmpty())
    val failed = result?.outcome == TelegramOutcome.Failed
    val label = when {
        failed -> stringResource(R.string.telegram_failed)
        status == TelegramItemState.Imported -> stringResource(R.string.telegram_imported)
        status == TelegramItemState.RestoreRequired -> stringResource(R.string.telegram_restore_required)
        status == TelegramItemState.OriginalMissing -> stringResource(R.string.telegram_redownload)
        item.animated -> stringResource(R.string.telegram_animated)
        else -> null
    }
    val selectable = status != TelegramItemState.RestoreRequired
    val scale by animateFloatAsState(if (selected) .9f else 1f, tween(MemeDockMotion.Feedback), label = "choice-scale")
    val highlight by animateColorAsState(if (selected) colors.primary.copy(alpha = .12f) else colors.primary.copy(alpha = 0f),
        tween(MemeDockMotion.Feedback), label = "choice-highlight")
    Box(Modifier.aspectRatio(1f).clip(MaterialTheme.shapes.small).background(highlight)
        .toggleable(selected, enabled = enabled && selectable, role = Role.Checkbox, onValueChange = { model.toggle(item.id) })
        .semantics { contentDescription = listOfNotNull(description, label).joinToString(", ") }
        .padding(MemeDockLayout.GapXSmall)) {
        Box(Modifier.fillMaxSize().graphicsLayer { scaleX = scale; scaleY = scale }, contentAlignment = Alignment.Center) {
            when {
                preview.path != null -> {
                    val request = remember(preview.path, context) { ImageRequest.Builder(context).data(File(preview.path!!)).size(256, 256).build() }
                    AsyncImage(request, description, imageLoader, Modifier.fillMaxSize())
                }
                preview.failed -> TextButton(onClick = { attempt++ }, enabled = enabled) { Text(stringResource(R.string.retry)) }
                !item.hasPreview -> Text(item.emoji.orEmpty().ifBlank { stringResource(R.string.telegram_no_preview) },
                    style = MaterialTheme.typography.bodySmall)
                else -> MemeDockSkeleton(Modifier.fillMaxSize(), MaterialTheme.shapes.small)
            }
            if (label != null) Text(label, Modifier.align(Alignment.BottomStart)
                .background(if (failed) colors.error else Color.Black.copy(alpha = .45f), MaterialTheme.shapes.extraSmall)
                .padding(horizontal = 6.dp, vertical = 2.dp),
                style = MaterialTheme.typography.labelSmall, color = if (failed) colors.onError else Color.White,
                maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (selectable) MemeDockSelectionMark(selected, Modifier.align(Alignment.TopEnd))
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
