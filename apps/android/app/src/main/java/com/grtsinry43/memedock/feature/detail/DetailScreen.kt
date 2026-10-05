package com.grtsinry43.memedock.feature.detail

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.library.StickerDetails
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun DetailScreen(state: DetailUiState, loader: ImageLoader, back: () -> Unit, retry: () -> Unit,
    togglePlayback: () -> Unit, cancelShare: () -> Unit, share: () -> Unit, initialItem: LibraryItem? = null,
    edit: (() -> Unit)? = null, organize: (() -> Unit)? = null, favorite: (() -> Unit)? = null,
    delete: (() -> Unit)? = null, restore: (() -> Unit)? = null) {
    val detail = state.detail
    val placeholder = remember(initialItem) { initialItem?.let {
        StickerDetails(it.id, it.title, "", it.originalName, it.mime, it.width, it.height, 0, it.animated,
            false, emptyList(), emptyList(), null, null)
    } }
    var fileInfo by rememberSaveable(detail?.id) { mutableStateOf(false) }
    Scaffold(containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(title = { Text(stringResource(R.string.detail_title), style = MaterialTheme.typography.titleMedium) },
                navigationIcon = { IconButton(onClick = back) { Icon(MemeDockIcons.Back, stringResource(R.string.back_to_library)) } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background))
        },
        bottomBar = {
            if (detail != null && !detail.deleted && state.error == null) Surface(color = MaterialTheme.colorScheme.surface, tonalElevation = 2.dp) {
                Column(Modifier.navigationBarsPadding().padding(horizontal = 20.dp, vertical = 12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    if (state.sharing) {
                        LinearProgressIndicator(Modifier.widthIn(max = MemeDockLayout.ContentWidth).fillMaxWidth())
                        Row(Modifier.widthIn(max = MemeDockLayout.ContentWidth).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                            Text(stringResource(R.string.share_preparing), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
                            TextButton(onClick = cancelShare) { Text(stringResource(R.string.cancel)) }
                        }
                    } else Button(onClick = share, enabled = !detail.deleted && detail.originalError == null,
                        shape = MaterialTheme.shapes.large, contentPadding = PaddingValues(16.dp),
                        modifier = Modifier.widthIn(max = MemeDockLayout.ContentWidth).fillMaxWidth()) {
                        Icon(MemeDockIcons.Share, null, Modifier.size(20.dp)); Spacer(Modifier.width(8.dp))
                        Text(stringResource(R.string.share_original))
                    }
                    state.shareError?.let { Text(if (state.shareLaunched) stringResource(R.string.share_accounting_failed) else failureText(it),
                        color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(top = 8.dp)) }
                }
            }
        }) { padding ->
        Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.TopCenter) {
            Column(Modifier.widthIn(max = MemeDockLayout.ContentWidth).fillMaxSize().verticalScroll(rememberScrollState())
                .padding(MemeDockLayout.PagePadding), verticalArrangement = Arrangement.spacedBy(20.dp)) {
                val shown = detail ?: placeholder
                if (shown != null) {
                    if (shown.previewPath != null || initialItem?.thumbnailPath != null)
                        StickerPreview(shown, loader, state.playing, initialItem?.thumbnailPath)
                    else if (!state.loading) MemeDockEmptyState(stringResource(R.string.preview_unavailable),
                        failureText(shown.originalError ?: "NOT_FOUND"), stringResource(R.string.retry), retry, icon = MemeDockIcons.Image)
                }
                when {
                    state.error != null -> MemeDockEmptyState(stringResource(R.string.detail_load_title), failureText(state.error),
                        stringResource(R.string.retry), retry, icon = MemeDockIcons.Alert)
                    state.loading -> Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 3.dp)
                    }
                    detail != null -> {
                        state.managementError?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error) }
                        if (state.managing) LinearProgressIndicator(Modifier.fillMaxWidth())
                        if (!detail.deleted) FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            edit?.let { OutlinedButton(onClick = it, enabled = !state.managing && !state.sharing) { Text(stringResource(R.string.edit_sticker)) } }
                            organize?.let { OutlinedButton(onClick = it, enabled = !state.managing && !state.sharing) { Text(stringResource(R.string.organize)) } }
                            favorite?.let { FilterChip(selected = detail.starred, onClick = it, enabled = !state.managing && !state.sharing,
                                label = { Text(stringResource(if (detail.starred) R.string.unfavorite else R.string.favorite)) }) }
                        }
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            Text(detail.title, style = MaterialTheme.typography.headlineSmall, modifier = Modifier.testTag("detail-name"))
                            if (detail.originalError != null && initialItem?.thumbnailPath != null) {
                                Text(failureText(detail.originalError), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                                TextButton(onClick = retry) { Text(stringResource(R.string.preview_retry)) }
                            }
                            if (detail.animated && detail.mime != "image/png") FilledTonalButton(onClick = togglePlayback) {
                                Icon(if (state.playing) MemeDockIcons.Pause else MemeDockIcons.Play, null, Modifier.size(18.dp))
                                Spacer(Modifier.width(8.dp))
                                Text(stringResource(if (state.playing) R.string.pause_animation else R.string.play_animation))
                            }
                            if (detail.animated && detail.mime == "image/png") Text(stringResource(R.string.apng_first_frame),
                                color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodySmall)
                            if (detail.note.isNotBlank()) Text(detail.note, style = MaterialTheme.typography.bodyLarge)
                        }
                        if (detail.tags.isNotEmpty()) DetailLabels(stringResource(R.string.detail_tags_title), detail.tags.map { it.name })
                        if (detail.collections.isNotEmpty()) DetailLabels(stringResource(R.string.detail_collections_title), detail.collections.map { it.name })
                        Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow) {
                            Column(Modifier.fillMaxWidth().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                                    Column(Modifier.weight(1f)) {
                                        Text(stringResource(R.string.detail_file_title), style = MaterialTheme.typography.titleSmall)
                                        Text(stringResource(R.string.detail_dimensions, detail.width, detail.height) + " · " + formatFileSize(detail.byteSize),
                                            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                    TextButton(onClick = { fileInfo = !fileInfo }) { Text(stringResource(if (fileInfo) R.string.collapse else R.string.expand)) }
                                }
                                AnimatedVisibility(fileInfo) {
                                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                                        Text(detail.originalName, style = MaterialTheme.typography.bodyMedium)
                                        Text(detail.mime.substringAfter('/').uppercase(), style = MaterialTheme.typography.labelMedium,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                }
                            }
                        }
                        if (detail.deleted) {
                            Text(stringResource(R.string.restore_hint), color = MaterialTheme.colorScheme.onSurfaceVariant)
                            restore?.let { Button(onClick = it, enabled = !state.managing, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.restore)) } }
                        } else delete?.let {
                            TextButton(onClick = it, enabled = !state.managing && !state.sharing) {
                                Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
                            }
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun DetailLabels(title: String, labels: List<String>) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, style = MaterialTheme.typography.titleSmall)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            labels.forEach { label -> Surface(shape = MaterialTheme.shapes.small, color = MaterialTheme.colorScheme.secondaryContainer) {
                Text(label, Modifier.padding(horizontal = 12.dp, vertical = 8.dp), style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSecondaryContainer)
            } }
        }
    }
}
