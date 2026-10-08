package com.grtsinry43.memedock.feature.organize

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import coil3.ImageLoader
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryCollection
import com.grtsinry43.memedock.data.library.LibraryTag
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import sh.calvin.reorderable.ReorderableColumn

class OrganizeActions(
    val retry: () -> Unit,
    val openCollection: (LibraryCollection) -> Unit,
    val openTag: (LibraryTag) -> Unit,
    val more: (OrganizeItem) -> Unit,
    val edit: (OrganizeEdit?) -> Unit,
    val commit: (String) -> Unit,
    val move: (from: Int, to: Int) -> Unit,
    val finishReorder: () -> Unit,
    val viewAll: () -> Unit,
)

@Composable
fun OrganizeScreen(state: OrganizeState, reordering: Boolean, actions: OrganizeActions, contentPadding: PaddingValues, loader: ImageLoader,
    thumbnail: suspend (String) -> String, allCollections: Boolean = false, back: () -> Unit = {}) {
    if (allCollections && !reordering) { CollectionsScreen(state, actions, loader, thumbnail, back); return }
    Column(
        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).imePadding()
            .verticalScroll(rememberScrollState())
            .windowInsetsPadding(WindowInsets.statusBars)
            .padding(bottom = contentPadding.calculateBottomPadding() + MemeDockLayout.SectionGap),
    ) {
        MemeDockPageHeader(stringResource(R.string.tab_organize)) {
            if (reordering) TextButton(onClick = actions.finishReorder, modifier = Modifier.testTag("organize-reorder-done")) {
                Text(stringResource(R.string.done))
            }
        }
        when {
            state.loading -> {
                MemeDockSectionHeader(stringResource(R.string.tab_collections))
                Row(Modifier.padding(horizontal = MemeDockLayout.PagePadding), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    repeat(3) { CollectionCardSkeleton(Modifier.width(CollectionCardWidth)) }
                }
            }
            state.error != null -> Box(Modifier.fillMaxWidth().padding(top = 48.dp), contentAlignment = Alignment.Center) {
                MemeDockEmptyState(stringResource(R.string.organize_load_failed), failureText(state.error),
                    stringResource(R.string.retry), actions.retry, icon = MemeDockIcons.Alert)
            }
            else -> {
                if (reordering) ReorderCollections(state.collections, actions.move)
                else Collections(state, actions, loader, thumbnail)
                Spacer(Modifier.height(MemeDockLayout.SectionGap))
                Tags(state, actions)
            }
        }
    }
}

@Composable
private fun Collections(state: OrganizeState, actions: OrganizeActions, loader: ImageLoader, thumbnail: suspend (String) -> String) {
    MemeDockSectionHeader(stringResource(R.string.tab_collections), action = stringResource(R.string.view_all_collections),
        onAction = actions.viewAll)
    val summaries = remember(state.summaries) { state.summaries.associateBy { it.collection.id } }
    LazyRow(contentPadding = PaddingValues(horizontal = MemeDockLayout.PagePadding), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        items(state.collections, key = { it.id }) { collection ->
            val summary = summaries[collection.id] ?: return@items
            CollectionCard(summary, loader, thumbnail, { actions.openCollection(collection) },
                { actions.more(OrganizeItem.Collection(collection)) }, Modifier.width(CollectionCardWidth))
        }
        if (state.editing != OrganizeEdit.NewCollection) item(key = "new") {
            NewCollectionCard({ actions.edit(OrganizeEdit.NewCollection) }, Modifier.width(CollectionCardWidth))
        }
    }
    CollectionEditor(state, actions, Modifier.padding(horizontal = MemeDockLayout.PagePadding).padding(top = MemeDockLayout.GapMedium))
}

@Composable
private fun ReorderCollections(collections: List<LibraryCollection>, move: (Int, Int) -> Unit) {
    val haptics = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    val corners = MaterialTheme.shapes.large
    val moveUp = stringResource(R.string.move_up)
    val moveDown = stringResource(R.string.move_down)
    MemeDockSectionHeader(stringResource(R.string.tab_collections), inset = MemeDockLayout.PagePadding + 16.dp)
    ReorderableColumn(
        list = collections,
        onSettle = move,
        onMove = { haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick) },
        modifier = Modifier.fillMaxWidth().padding(horizontal = MemeDockLayout.PagePadding),
    ) { index, collection, dragging ->
        key(collection.id) {
            ReorderableItem {
                val elevation by animateDpAsState(if (dragging) 6.dp else 0.dp, label = "reorder-elevation")
                val shape = when {
                    dragging || collections.size == 1 -> corners
                    index == 0 -> corners.copy(bottomStart = CornerSize(0), bottomEnd = CornerSize(0))
                    index == collections.lastIndex -> corners.copy(topStart = CornerSize(0), topEnd = CornerSize(0))
                    else -> RectangleShape
                }
                Surface(shape = shape, color = colors.surfaceContainerLowest, shadowElevation = elevation,
                    modifier = Modifier.semantics {
                        customActions = buildList {
                            if (index > 0) add(CustomAccessibilityAction(moveUp) { move(index, index - 1); true })
                            if (index < collections.lastIndex) add(CustomAccessibilityAction(moveDown) { move(index, index + 1); true })
                        }
                    }) {
                    Column {
                        if (index > 0 && !dragging) HorizontalDivider(Modifier.padding(start = 16.dp), MemeDockLayout.Hairline,
                            colors.outlineVariant)
                        Row(Modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight).padding(start = 16.dp, end = 4.dp),
                            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.GapLarge)) {
                            Icon(MemeDockIcons.Folder, null, Modifier.size(MemeDockLayout.IconLarge), tint = colors.onSurfaceVariant)
                            Text(collection.name, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge,
                                maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Icon(MemeDockIcons.DragHandle, null,
                                Modifier.size(48.dp).clearAndSetSemantics {}
                                    .draggableHandle(
                                        onDragStarted = { haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate) },
                                        onDragStopped = { haptics.performHapticFeedback(HapticFeedbackType.GestureEnd) },
                                    )
                                    .testTag("reorder-handle:${collection.id}")
                                    .padding(12.dp),
                                tint = colors.onSurfaceVariant)
                        }
                    }
                }
            }
        }
    }
    SectionFooter(stringResource(R.string.organize_reorder_hint))
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Tags(state: OrganizeState, actions: OrganizeActions) {
    val editing = state.editing
    MemeDockSectionHeader(stringResource(R.string.tags_title))
    FlowRow(Modifier.fillMaxWidth().padding(horizontal = MemeDockLayout.PagePadding),
        horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        state.tags.forEach { tag ->
            MemeDockChip(tag.name, Modifier.testTag("tag:${tag.id}"), accent = (editing as? OrganizeItem.Tag)?.value?.id == tag.id,
                container = MaterialTheme.colorScheme.surfaceContainerLowest,
                onLongClick = { actions.more(OrganizeItem.Tag(tag)) }, onClick = { actions.openTag(tag) })
        }
        if (editing != OrganizeEdit.NewTag) MemeDockAddChip(stringResource(R.string.new_tag), { actions.edit(OrganizeEdit.NewTag) },
            Modifier.testTag("organize-new-tag"))
    }
    if (editing == OrganizeEdit.NewTag || editing is OrganizeItem.Tag) key(editing) {
        MemeDockInlineEdit(
            initial = (editing as? OrganizeItem.Tag)?.value?.name.orEmpty(),
            placeholder = stringResource(if (editing is OrganizeItem.Tag) R.string.name else R.string.new_tag),
            onCommit = actions.commit,
            onCancel = { actions.edit(null) },
            modifier = Modifier.padding(horizontal = MemeDockLayout.PagePadding).padding(top = 12.dp),
            busy = state.busy,
            error = state.editError?.let { failureText(it) },
        )
    }
    SectionFooter(stringResource(if (state.tags.isEmpty()) R.string.organize_tags_empty else R.string.organize_tags_footer))
}

@Composable
private fun SectionFooter(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = MemeDockLayout.PagePadding).padding(top = 8.dp))
}
