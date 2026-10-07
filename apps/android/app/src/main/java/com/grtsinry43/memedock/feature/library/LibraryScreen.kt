package com.grtsinry43.memedock.feature.library

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import dev.chrisbanes.haze.rememberHazeState
import kotlinx.coroutines.flow.distinctUntilChanged
import sh.calvin.reorderable.ReorderableItem
import sh.calvin.reorderable.ReorderableLazyGridState
import sh.calvin.reorderable.rememberReorderableLazyGridState

private val GridGutter = 12.dp

/** Callbacks shared by every sticker grid. */
class StickerGridActions(
    val thumbnail: (LibraryItem, Boolean) -> Unit,
    val open: (LibraryItem) -> Unit,
    /** Long press; null where a grid offers no quick actions. */
    val more: ((LibraryItem) -> Unit)?,
    val loadMore: () -> Unit,
    val retry: () -> Unit,
    val selection: BatchSelectionState? = null,
    val startSelection: (() -> Unit)? = null,
    val exitSelection: (() -> Unit)? = null,
    val organizeSelection: (() -> Unit)? = null,
)

/** Import work shown above the grid while it is in progress or waiting for the user. */
sealed interface ImportNotice {
    data class Preparing(val stopping: Boolean) : ImportNotice
    data class Running(val done: Int, val total: Int, val stopping: Boolean) : ImportNotice
    data class Waiting(val count: Int) : ImportNotice
}

@Composable
fun HomeLibraryScreen(
    state: LibraryUiState,
    imageLoader: ImageLoader,
    actions: StickerGridActions,
    search: (String) -> Unit,
    select: (LibraryFilter) -> Unit,
    add: () -> Unit,
    notice: ImportNotice?,
    showImports: () -> Unit,
    stopImports: () -> Unit,
    contentPadding: PaddingValues,
) {
    val grid = rememberLazyGridState()
    val focus = LocalFocusManager.current
    LaunchedEffect(grid) {
        snapshotFlow { grid.isScrollInProgress }.collect { if (it) focus.clearFocus() }
    }
    StickerGrid(state, imageLoader, actions, grid,
        Modifier.background(MaterialTheme.colorScheme.background).windowInsetsPadding(WindowInsets.statusBars),
        contentPadding,
        header = {
            item(key = "title", span = { GridItemSpan(maxLineSpan) }, contentType = "title") {
                Row(Modifier.fillMaxWidth().padding(start = 4.dp, top = 12.dp, bottom = 4.dp),
                    verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.tab_stickers), style = MaterialTheme.typography.headlineLarge,
                        modifier = Modifier.weight(1f).semantics { heading() })
                    SelectionModeButton(actions)
                    FilledTonalIconButton(onClick = add, enabled = actions.selection?.busy != true,
                        modifier = Modifier.testTag("library-add")) {
                        Icon(MemeDockIcons.Add, stringResource(R.string.library_add), Modifier.size(24.dp))
                    }
                }
            }
            stickyHeader(key = "controls", contentType = "controls") {
                Column(Modifier.fillMaxWidth().bleed(GridGutter).background(MaterialTheme.colorScheme.background)
                    .padding(horizontal = GridGutter, vertical = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    SearchField(state.search, search)
                    FilterRow(state, select)
                    val shownNotice = rememberRetained(notice)
                    AnimatedVisibility(notice != null) { shownNotice?.let { ImportNoticeRow(it, showImports, stopImports) } }
                }
            }
        },
        empty = {
            val filter = state.filter
            when {
                state.search.isNotBlank() -> MemeDockEmptyState(stringResource(R.string.library_no_matches),
                    stringResource(R.string.library_no_matches_hint), stringResource(R.string.clear_search), { search("") },
                    icon = MemeDockIcons.Search)
                filter == LibraryFilter.Starred -> MemeDockEmptyState(stringResource(R.string.starred_empty),
                    stringResource(R.string.starred_empty_hint), icon = MemeDockIcons.Star)
                filter is LibraryFilter.Collection -> MemeDockEmptyState(stringResource(R.string.collection_empty),
                    stringResource(R.string.collection_empty_hint), icon = MemeDockIcons.Collections)
                else -> MemeDockEmptyState(stringResource(R.string.library_empty), stringResource(R.string.library_empty_hint),
                    stringResource(R.string.import_photos), add, icon = MemeDockIcons.Mood)
            }
        },
    )
}

/** Page-level callbacks of a collection or tag page. */
class GroupPageActions(
    val back: () -> Unit,
    val more: () -> Unit,
    val rename: (String) -> Unit,
    val cancelRename: () -> Unit,
    val finishReorder: () -> Unit,
    /** [order] is the full list as dropped, [moved] the tile that was dragged. */
    val reorder: (order: List<LibraryItem>, moved: LibraryItem) -> Unit,
)

/** A collection or tag page. While [reordering], tiles take no taps and a long press drags them. */
@Composable
fun GroupLibraryScreen(
    group: GroupState,
    state: LibraryUiState,
    imageLoader: ImageLoader,
    actions: StickerGridActions,
    page: GroupPageActions,
    reordering: Boolean,
    empty: @Composable () -> Unit,
) {
    val grid = rememberLazyGridState()
    val glass = rememberHazeState()
    val haptics = LocalHapticFeedback.current
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + MemeDockLayout.TopBarHeight
    val bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    // The drag works on a local copy so each step lands before the library confirms it.
    var order by remember { mutableStateOf(state.items) }
    var dragged by remember { mutableStateOf<LibraryItem?>(null) }
    LaunchedEffect(state.items) { if (dragged == null) order = state.items }
    val reorderState = rememberReorderableLazyGridState(grid) { from, to ->
        val source = order.indexOfFirst { it.id == from.key }
        val target = order.indexOfFirst { it.id == to.key }
        if (source >= 0 && target >= 0) {
            order = order.toMutableList().apply { add(target, removeAt(source)) }
            haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick)
        }
    }
    fun dropped() {
        val moved = dragged ?: return
        dragged = null
        // Thumbnails that arrived mid-drag are merged in rather than lost.
        val latest = state.items.associateBy(LibraryItem::id)
        order = order.map { latest[it.id] ?: it }
        page.reorder(order, moved)
    }
    LaunchedEffect(group.renaming) { if (group.renaming) grid.animateScrollToItem(0) }
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        StickerGrid(state, imageLoader, actions, grid, Modifier.glassSource(glass),
            PaddingValues(top = top + 8.dp, bottom = bottom),
            reorder = if (reordering) GridReorder(reorderState, order, enabled = !state.hasMore,
                started = { dragged = it }, stopped = ::dropped) else null,
            header = {
                item(key = "group-title", span = { GridItemSpan(maxLineSpan) }, contentType = "title") {
                    if (group.renaming) MemeDockInlineEdit(group.name, stringResource(R.string.name), page.rename, page.cancelRename,
                        Modifier.padding(top = 12.dp, bottom = 8.dp).testTag("group-rename"), busy = group.busy,
                        error = group.editError?.let { failureText(it) })
                    else Text(group.name, style = MaterialTheme.typography.headlineLarge,
                        modifier = Modifier.padding(start = 4.dp, end = 4.dp, top = 12.dp, bottom = 8.dp).semantics { heading() })
                }
                if (reordering) item(key = "reorder-hint", span = { GridItemSpan(maxLineSpan) }, contentType = "hint") {
                    Row(Modifier.padding(start = 4.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (state.hasMore) CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 2.dp)
                        Text(stringResource(if (state.hasMore) R.string.reorder_loading else R.string.reorder_grid_hint),
                            style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            },
            empty = empty,
        )
        val scrolled by remember { derivedStateOf { grid.canScrollBackward } }
        val titled by remember { derivedStateOf { grid.firstVisibleItemIndex > 0 } }
        MemeDockTopBar(if (titled) group.name else "", glass, back = page.back, scrolled = scrolled) {
            if (reordering) TextButton(onClick = page.finishReorder, modifier = Modifier.testTag("group-reorder-done")) {
                Text(stringResource(R.string.done))
            } else {
                SelectionModeButton(actions)
                IconButton(onClick = page.more, enabled = !group.busy && actions.selection?.busy != true, modifier = Modifier.testTag("group-more")) {
                    Icon(MemeDockIcons.More, stringResource(R.string.more_actions))
                }
            }
        }
    }
}

/** Drag state for a grid showing [items] in place of the loaded page; drags start only while [enabled]. */
internal class GridReorder(
    val state: ReorderableLazyGridState,
    val items: List<LibraryItem>,
    val enabled: Boolean,
    val started: (LibraryItem) -> Unit,
    val stopped: () -> Unit,
)

/** The paged sticker grid behind every sticker page; [header] items come first and [empty] shows when nothing matched. */
@Composable
internal fun StickerGrid(
    state: LibraryUiState,
    imageLoader: ImageLoader,
    actions: StickerGridActions,
    grid: LazyGridState,
    modifier: Modifier,
    contentPadding: PaddingValues,
    header: LazyGridScope.() -> Unit,
    empty: @Composable () -> Unit,
    reorder: GridReorder? = null,
) {
    // Reordering needs every page: the last loaded tile has no known successor to drop in front of.
    val eager = reorder != null
    LaunchedEffect(grid, state.items.size, state.hasMore, state.loadingMore, state.pageError, eager) {
        snapshotFlow { grid.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: -1 }.distinctUntilChanged().collect { last ->
            if (state.hasMore && !state.loadingMore && state.pageError == null &&
                (eager || last >= grid.layoutInfo.totalItemsCount - 12)) actions.loadMore()
        }
    }
    val glass = rememberHazeState()
    val selecting = actions.selection?.selecting == true
    Box(modifier.fillMaxSize()) {
        LazyVerticalGrid(
            columns = GridCells.Adaptive(104.dp),
            state = grid,
            modifier = Modifier.fillMaxSize().glassSource(glass).testTag("library-grid"),
            contentPadding = PaddingValues(
                start = GridGutter, end = GridGutter,
                top = contentPadding.calculateTopPadding(),
                bottom = contentPadding.calculateBottomPadding() + if (selecting) 72.dp else 16.dp,
            ),
            horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.GapSmall),
            verticalArrangement = Arrangement.spacedBy(MemeDockLayout.GapSmall),
        ) {
            header()
            when {
                state.loading -> items(12, key = { "skeleton:$it" }, contentType = { "skeleton" }) {
                    MemeDockSkeleton(Modifier.aspectRatio(1f))
                }
                state.error != null -> item(key = "error", span = { GridItemSpan(maxLineSpan) }) {
                    Box(Modifier.fillMaxWidth().padding(top = 48.dp), contentAlignment = Alignment.Center) {
                        MemeDockEmptyState(stringResource(R.string.library_load_title), failureText(state.error),
                            stringResource(R.string.retry), actions.retry, icon = MemeDockIcons.Alert)
                    }
                }
                state.items.isEmpty() -> item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                    Box(Modifier.fillMaxWidth().padding(top = 48.dp), contentAlignment = Alignment.Center) { empty() }
                }
                else -> {
                    items(reorder?.items ?: state.items, key = LibraryItem::id, contentType = { "sticker" }) { item ->
                        LaunchedEffect(item.id, item.thumbnailPath, item.thumbnailState, state.thumbnailEpoch) { actions.thumbnail(item, false) }
                        if (reorder == null) {
                            LibraryTile(item, imageLoader, { actions.thumbnail(item, true) }, { actions.open(item) },
                                actions.more?.let { more -> { more(item) } }, Modifier.animateItem(),
                                selected = item.id in actions.selection?.selected.orEmpty(), selecting = actions.selection?.selecting == true)
                        } else ReorderableItem(reorder.state, key = item.id) { dragging ->
                            val scale by animateFloatAsState(if (dragging) 1.06f else 1f, label = "drag-scale")
                            LibraryTile(item, imageLoader, { actions.thumbnail(item, true) }, null, null,
                                Modifier.graphicsLayer { scaleX = scale; scaleY = scale }.longPressDraggableHandle(
                                    enabled = reorder.enabled,
                                    onDragStarted = { reorder.started(item) },
                                    onDragStopped = reorder.stopped,
                                ))
                        }
                    }
                    if (state.loadingMore || state.pageError != null) item(key = "footer", span = { GridItemSpan(maxLineSpan) }) {
                        Box(Modifier.fillMaxWidth().heightIn(min = 56.dp), contentAlignment = Alignment.Center) {
                            if (state.loadingMore) CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                            else TextButton(onClick = actions.loadMore) { Text(stringResource(R.string.library_page_error)) }
                        }
                    }
                }
            }
        }
        AnimatedVisibility(selecting,
            modifier = Modifier.align(Alignment.BottomCenter).padding(horizontal = 16.dp)
                .padding(bottom = contentPadding.calculateBottomPadding() + 12.dp),
            enter = fadeIn() + slideInVertically { it / 2 }, exit = fadeOut() + slideOutVertically { it / 2 }) {
            actions.selection?.let { selection -> SelectionBar(selection, glass,
                organize = { actions.organizeSelection?.invoke() }, exit = { actions.exitSelection?.invoke() }) }
        }
    }
}

@Composable
private fun SearchField(value: String, search: (String) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val focus = LocalFocusManager.current
    BasicTextField(
        value = value,
        onValueChange = search,
        modifier = Modifier.fillMaxWidth().testTag("library-search"),
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = colors.onSurface),
        cursorBrush = SolidColor(colors.primary),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { focus.clearFocus() }),
        decorationBox = { field ->
            Row(Modifier.fillMaxWidth().height(44.dp).background(colors.surfaceContainer, MaterialTheme.shapes.small)
                .padding(start = 12.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(MemeDockIcons.Search, null, Modifier.size(20.dp), tint = colors.onSurfaceVariant)
                Spacer(Modifier.width(8.dp))
                Box(Modifier.weight(1f)) {
                    if (value.isEmpty()) Text(stringResource(R.string.library_search), style = MaterialTheme.typography.bodyLarge,
                        color = colors.onSurfaceVariant)
                    field()
                }
                if (value.isNotEmpty()) IconButton(onClick = { search("") }, Modifier.size(36.dp)) {
                    Icon(MemeDockIcons.Close, stringResource(R.string.clear_search), Modifier.size(18.dp), tint = colors.onSurfaceVariant)
                }
            }
        },
    )
}

@Composable
private fun FilterRow(state: LibraryUiState, select: (LibraryFilter) -> Unit) {
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        FilterPill(stringResource(R.string.filter_recent), state.filter == LibraryFilter.Recent) { select(LibraryFilter.Recent) }
        FilterPill(stringResource(R.string.filter_all), state.filter == LibraryFilter.All) { select(LibraryFilter.All) }
        FilterPill(stringResource(R.string.favorites_only), state.filter == LibraryFilter.Starred) { select(LibraryFilter.Starred) }
        state.collections.forEach { collection ->
            val filter = LibraryFilter.Collection(collection.id)
            FilterPill(collection.name, state.filter == filter) { select(filter) }
        }
    }
}

/** Widens a full-span grid item over the grid's side padding so pinned content hides what scrolls past. */
private fun Modifier.bleed(horizontal: Dp) = layout { measurable, constraints ->
    val extra = horizontal.roundToPx()
    val placeable = measurable.measure(constraints.copy(
        minWidth = constraints.minWidth + extra * 2, maxWidth = constraints.maxWidth + extra * 2))
    layout(constraints.maxWidth, placeable.height) { placeable.place(-extra, 0) }
}

@Composable
private fun FilterPill(label: String, selected: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Text(
        label,
        style = MaterialTheme.typography.labelLarge,
        color = if (selected) colors.surfaceContainerLowest else colors.onSurface,
        maxLines = 1,
        modifier = Modifier.height(32.dp)
            .background(if (selected) colors.onSurface else colors.surfaceContainer, CircleShape)
            .clip(CircleShape)
            .selectable(selected, role = Role.Tab, onClick = onClick)
            .padding(horizontal = 14.dp)
            .wrapContentHeight(Alignment.CenterVertically),
    )
}

@Composable
private fun ImportNoticeRow(notice: ImportNotice, open: () -> Unit, stop: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val waiting = notice is ImportNotice.Waiting
    Row(
        Modifier.fillMaxWidth().heightIn(min = 44.dp).clip(MaterialTheme.shapes.small).background(colors.primary.copy(alpha = .10f))
            .then(if (waiting) Modifier.clickable(role = Role.Button, onClick = open) else Modifier)
            .padding(start = 14.dp, end = if (waiting) 10.dp else 4.dp).testTag("import-notice"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        when (notice) {
            is ImportNotice.Preparing -> CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
            is ImportNotice.Running -> CircularProgressIndicator(
                progress = { if (notice.total == 0) 0f else notice.done.toFloat() / notice.total },
                modifier = Modifier.size(16.dp), strokeWidth = 2.dp, trackColor = colors.primary.copy(alpha = .2f))
            is ImportNotice.Waiting -> Icon(MemeDockIcons.Image, null, Modifier.size(18.dp), tint = colors.primary)
        }
        Text(
            when (notice) {
                is ImportNotice.Preparing -> stringResource(R.string.import_preparing)
                is ImportNotice.Running -> stringResource(R.string.import_running_progress, notice.done, notice.total)
                is ImportNotice.Waiting -> stringResource(R.string.import_waiting_count, notice.count)
            },
            style = MaterialTheme.typography.bodyMedium, color = colors.onSurface, modifier = Modifier.weight(1f),
        )
        val stopping = (notice as? ImportNotice.Running)?.stopping ?: (notice as? ImportNotice.Preparing)?.stopping
        if (stopping == null) Icon(MemeDockIcons.ChevronRight, null, Modifier.size(18.dp), tint = colors.onSurfaceVariant)
        else TextButton(onClick = stop, enabled = !stopping) {
            Text(stringResource(if (stopping) R.string.import_cancelling else R.string.import_cancel_remaining))
        }
    }
}
