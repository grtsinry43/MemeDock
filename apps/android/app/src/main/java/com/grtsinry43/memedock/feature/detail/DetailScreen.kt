package com.grtsinry43.memedock.feature.detail

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.library.StickerDetails
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import com.grtsinry43.memedock.ui.navigation.detailElementTransition
import dev.chrisbanes.haze.rememberHazeState

enum class DetailField { Title, Note }

class DetailActions(
    val back: () -> Unit,
    val retry: () -> Unit,
    val togglePlayback: () -> Unit,
    val share: (firstFrame: Boolean) -> Unit,
    val copy: () -> Unit,
    val save: () -> Unit,
    val cancelOutput: () -> Unit,
    val choosePreset: () -> Unit,
    val star: () -> Unit,
    val organize: () -> Unit,
    val more: () -> Unit,
    val restore: () -> Unit,
    val edit: (DetailField?) -> Unit,
    val commit: (DetailField, String) -> Unit,
)

@Composable
fun DetailScreen(
    state: DetailUiState,
    loader: ImageLoader,
    initialItem: LibraryItem?,
    choice: ExportChoice,
    outputsReady: Boolean,
    editing: DetailField?,
    actions: DetailActions,
) {
    val detail = state.detail
    val placeholder = remember(initialItem) { initialItem?.let {
        StickerDetails(it.id, it.title, "", it.originalName, it.mime, it.width, it.height, 0, it.animated,
            false, emptyList(), null, null, null)
    } }
    val shown = detail ?: placeholder
    val glass = rememberHazeState()
    val scroll = rememberScrollState()
    val density = LocalDensity.current
    var barHeight by remember { mutableStateOf(0.dp) }
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + MemeDockLayout.TopBarHeight
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        Column(
            Modifier.fillMaxSize().glassSource(glass).imePadding().verticalScroll(scroll)
                .padding(top = top + MemeDockLayout.GapSmall, bottom = barHeight + MemeDockLayout.SectionGap),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Column(Modifier.widthIn(max = MemeDockLayout.ContentWidth).fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(MemeDockLayout.SectionGap)) {
                if (shown != null) Box(Modifier.padding(horizontal = MemeDockLayout.PagePadding)) {
                    if (shown.previewPath != null || initialItem?.thumbnailPath != null)
                        StickerPreview(shown, loader, state.playing, initialItem?.thumbnailPath)
                    else if (!state.loading) MemeDockEmptyState(stringResource(R.string.preview_unavailable),
                        failureText(shown.originalError ?: "NOT_FOUND"), stringResource(R.string.retry), actions.retry,
                        icon = MemeDockIcons.Image)
                }
                Column(Modifier.fillMaxWidth().detailElementTransition(fromTop = false),
                    verticalArrangement = Arrangement.spacedBy(MemeDockLayout.SectionGap)) {
                    when {
                        state.error != null -> MemeDockEmptyState(stringResource(R.string.detail_load_title), failureText(state.error),
                            stringResource(R.string.retry), actions.retry, icon = MemeDockIcons.Alert)
                        detail == null -> LoadingDetail()
                        else -> DetailBody(detail, state, editing, actions, hasThumbnail = initialItem?.thumbnailPath != null)
                    }
                }
            }
        }
        if (shown != null && state.error == null) DetailBar(
            shown, state, choice, outputsReady && detail != null, actions,
            Modifier.align(Alignment.BottomCenter).detailElementTransition(fromTop = false).glass(glass, glassStyle())
                .onSizeChanged { barHeight = with(density) { it.height.toDp() } },
        )
        val scrolled by remember { derivedStateOf { scroll.value > 0 } }
        // The page title appears once the sticker's own name has scrolled under the bar.
        val titled by remember { derivedStateOf { scroll.value > with(density) { 320.dp.roundToPx() } } }
        MemeDockTopBar(if (titled) detail?.title.orEmpty() else "", glass,
            modifier = Modifier.detailElementTransition(fromTop = true), back = actions.back, scrolled = scrolled) {
            if (detail != null && !detail.deleted) {
                IconButton(onClick = actions.star, enabled = !state.managing, modifier = Modifier.testTag("detail-star")) {
                    Icon(if (detail.starred) MemeDockIcons.StarFilled else MemeDockIcons.Star, stringResource(if (detail.starred) R.string.unfavorite else R.string.favorite),
                        tint = if (detail.starred) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onSurfaceVariant)
                }
                IconButton(onClick = actions.more, enabled = !state.managing && !state.sharing,
                    modifier = Modifier.testTag("detail-more")) {
                    Icon(MemeDockIcons.More, stringResource(R.string.more_actions))
                }
            }
        }
    }
}

@Composable
private fun LoadingDetail() {
    Column(Modifier.padding(horizontal = MemeDockLayout.PagePadding), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        MemeDockSkeleton(Modifier.fillMaxWidth(.5f).height(24.dp), MaterialTheme.shapes.extraSmall)
        MemeDockSkeleton(Modifier.fillMaxWidth(.3f).height(16.dp), MaterialTheme.shapes.extraSmall)
        Spacer(Modifier.height(8.dp))
        MemeDockSkeleton(Modifier.fillMaxWidth().height(168.dp), MaterialTheme.shapes.large)
    }
}

@Composable
private fun DetailBody(detail: StickerDetails, state: DetailUiState, editing: DetailField?, actions: DetailActions, hasThumbnail: Boolean) {
    val colors = MaterialTheme.colorScheme
    val editable = !detail.deleted && !state.sharing
    val error = state.managementError?.let { failureText(it) }
    Column(Modifier.padding(horizontal = MemeDockLayout.PagePadding), verticalArrangement = Arrangement.spacedBy(MemeDockLayout.GapSmall)) {
        if (editing == DetailField.Title) MemeDockInlineEdit(detail.title, stringResource(R.string.sticker_title),
            { actions.commit(DetailField.Title, it) }, { actions.edit(null) }, busy = state.managing, error = error)
        else Row(verticalAlignment = Alignment.CenterVertically) {
            Text(detail.title, style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.weight(1f).clip(MaterialTheme.shapes.extraSmall)
                    .clickable(enabled = editable && editing == null, role = Role.Button) { actions.edit(DetailField.Title) }
                    .padding(vertical = 4.dp).testTag("detail-name"))
            if (detail.animated && detail.mime != "image/png") FilledTonalIconButton(onClick = actions.togglePlayback) {
                Icon(if (state.playing) MemeDockIcons.Pause else MemeDockIcons.Play,
                    stringResource(if (state.playing) R.string.pause_animation else R.string.play_animation))
            }
        }
        when {
            editing == DetailField.Note -> MemeDockInlineEdit(detail.note, stringResource(R.string.sticker_note),
                { actions.commit(DetailField.Note, it) }, { actions.edit(null) }, busy = state.managing, error = error,
                allowEmpty = true, multiline = true)
            detail.note.isNotBlank() -> Text(detail.note, style = MaterialTheme.typography.bodyLarge, color = colors.onSurfaceVariant,
                modifier = Modifier.fillMaxWidth().clip(MaterialTheme.shapes.extraSmall)
                    .clickable(enabled = editable && editing == null, role = Role.Button) { actions.edit(DetailField.Note) }
                    .padding(vertical = 4.dp))
            editable -> Text(stringResource(R.string.add_note), style = MaterialTheme.typography.bodyLarge, color = colors.primary,
                modifier = Modifier.clip(MaterialTheme.shapes.extraSmall)
                    .clickable(enabled = editing == null, role = Role.Button) { actions.edit(DetailField.Note) }
                    .padding(vertical = 4.dp).testTag("detail-add-note"))
        }
        if (detail.animated && detail.mime == "image/png") Text(stringResource(R.string.apng_first_frame),
            style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
        if (detail.originalError != null && hasThumbnail) MemeDockSheetError(failureText(detail.originalError), inset = 0.dp,
            retryLabel = stringResource(R.string.preview_retry), retry = actions.retry)
        if (detail.deleted) Text(stringResource(R.string.restore_hint), style = MaterialTheme.typography.bodyMedium,
            color = colors.onSurfaceVariant)
        else Relations(detail, enabled = editable && editing == null && !state.managing, actions.organize)
    }
    MemeDockGroup(title = stringResource(R.string.detail_file_title)) {
        row { MemeDockRow(stringResource(R.string.detail_size), value = stringResource(R.string.detail_dimensions, detail.width, detail.height)) }
        row { MemeDockRow(stringResource(R.string.detail_bytes), value = formatFileSize(detail.byteSize)) }
        row { MemeDockRow(stringResource(R.string.detail_format), value = detail.mime.substringAfter('/').uppercase()) }
        row { MemeDockRow(stringResource(R.string.detail_original_name), subtitle = detail.originalName) }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Relations(detail: StickerDetails, enabled: Boolean, organize: () -> Unit) {
    FlowRow(Modifier.fillMaxWidth().padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)) {
        val container = MaterialTheme.colorScheme.surfaceContainerLowest
        detail.collection?.let { MemeDockChip(it.name, icon = MemeDockIcons.Folder, enabled = enabled, container = container, onClick = organize) }
        detail.tags.forEach { MemeDockChip(it.name, icon = MemeDockIcons.Label, enabled = enabled, container = container, onClick = organize) }
        val empty = detail.collection == null && detail.tags.isEmpty()
        MemeDockAddChip(stringResource(if (empty) R.string.organize_hint else R.string.organize), organize,
            Modifier.testTag("detail-organize"), enabled = enabled)
    }
}

@Composable
private fun DetailBar(detail: StickerDetails, state: DetailUiState, choice: ExportChoice, outputsReady: Boolean,
    actions: DetailActions, modifier: Modifier) {
    val colors = MaterialTheme.colorScheme
    Column(modifier.fillMaxWidth()) {
        HorizontalDivider(thickness = MemeDockLayout.Hairline, color = colors.outlineVariant)
        Column(
            Modifier.align(Alignment.CenterHorizontally).navigationBarsPadding().widthIn(max = MemeDockLayout.ContentWidth)
                .fillMaxWidth().padding(horizontal = MemeDockLayout.PagePadding, vertical = MemeDockLayout.GapMedium),
            verticalArrangement = Arrangement.spacedBy(MemeDockLayout.GapSmall),
        ) {
            if (detail.deleted) {
                MemeDockButton(stringResource(R.string.restore), actions.restore, Modifier.testTag("restore-sticker"),
                    enabled = !state.managing)
                return@Column
            }
            val ready = outputsReady && !state.managing && detail.originalError == null
            // Animated stickers lose their motion in the other presets, so the main action sends the original.
            val firstFrameOnly = detail.animated && choice != ExportChoice.Original
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                PresetPill(choice, enabled = ready && !state.sharing, onClick = actions.choosePreset)
                Spacer(Modifier.weight(1f))
                if (state.sharing) TextButton(onClick = actions.cancelOutput) { Text(stringResource(R.string.cancel)) }
                else {
                    IconButton(onClick = actions.copy, enabled = ready, modifier = Modifier.testTag("copy-image")) {
                        Icon(MemeDockIcons.Copy, stringResource(R.string.copy_image))
                    }
                    IconButton(onClick = actions.save, enabled = ready, modifier = Modifier.testTag("save-image")) {
                        Icon(MemeDockIcons.Download, stringResource(R.string.save_image))
                    }
                }
            }
            MemeDockButton(
                stringResource(if (choice == ExportChoice.Original || firstFrameOnly) R.string.share_original else R.string.share_sticker),
                { actions.share(false) }, Modifier.testTag("share-sticker"), enabled = ready, busy = state.sharing,
                busyText = stringResource(if (state.saving) R.string.image_saving else R.string.share_preparing), icon = MemeDockIcons.Share,
            )
            if (firstFrameOnly) MemeDockTextButton(stringResource(R.string.share_first_frame_as, stringResource(choice.titleResource())),
                { actions.share(true) }, Modifier.testTag("share-first-frame"), enabled = ready && !state.sharing)
        }
    }
}

@Composable
private fun PresetPill(choice: ExportChoice, enabled: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.heightIn(min = MemeDockLayout.ChipHeight).clip(MaterialTheme.shapes.small).background(colors.surfaceContainer)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(start = 14.dp, end = 10.dp).testTag("choose-export-preset"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        val disabled = colors.onSurface.copy(alpha = .38f)
        Text(stringResource(choice.titleResource()), style = MaterialTheme.typography.bodyMedium,
            color = if (enabled) colors.onSurface else disabled)
        Icon(MemeDockIcons.ExpandMore, stringResource(R.string.export_choose), Modifier.size(MemeDockLayout.IconMedium),
            tint = if (enabled) colors.onSurfaceVariant else disabled)
    }
}
