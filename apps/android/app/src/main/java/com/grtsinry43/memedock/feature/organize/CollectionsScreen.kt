package com.grtsinry43.memedock.feature.organize

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import dev.chrisbanes.haze.rememberHazeState

@Composable
internal fun CollectionsScreen(state: OrganizeState, actions: OrganizeActions, loader: ImageLoader,
    thumbnail: suspend (String) -> String, back: () -> Unit) {
    val summaries = remember(state.summaries) { state.summaries.associateBy { it.collection.id } }
    val glass = rememberHazeState()
    val grid = rememberLazyGridState()
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + MemeDockLayout.TopBarHeight
    val bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding() + MemeDockLayout.SectionGap
    val newCollection = { actions.edit(OrganizeEdit.NewCollection) }
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).imePadding()
        .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))) {
        LazyVerticalGrid(GridCells.Adaptive(CollectionCardWidth), Modifier.fillMaxSize().glassSource(glass), grid,
            PaddingValues(start = MemeDockLayout.PagePadding, end = MemeDockLayout.PagePadding, top = top + MemeDockLayout.GapSmall, bottom = bottom),
            verticalArrangement = Arrangement.spacedBy(MemeDockLayout.GapMedium),
            horizontalArrangement = Arrangement.spacedBy(MemeDockLayout.GapMedium)) {
            if (state.editingCollection) item(key = "editor", span = { GridItemSpan(maxLineSpan) }) { CollectionEditor(state, actions) }
            when {
                state.loading -> items(6, key = { "skeleton-$it" }) { CollectionCardSkeleton() }
                state.error != null -> item(key = "error", span = { GridItemSpan(maxLineSpan) }) {
                    Box(Modifier.padding(top = 48.dp), contentAlignment = Alignment.Center) {
                        MemeDockEmptyState(stringResource(R.string.organize_load_failed), failureText(state.error),
                            stringResource(R.string.retry), actions.retry, icon = MemeDockIcons.Alert)
                    }
                }
                state.collections.isEmpty() -> item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                    Box(Modifier.padding(top = 48.dp), contentAlignment = Alignment.Center) {
                        MemeDockEmptyState(stringResource(R.string.collections_empty), stringResource(R.string.collections_empty_hint),
                            stringResource(R.string.new_collection).takeIf { state.editing != OrganizeEdit.NewCollection },
                            newCollection, icon = MemeDockIcons.Folder)
                    }
                }
                else -> items(state.collections, key = { it.id }) { collection ->
                    val summary = summaries[collection.id] ?: return@items
                    CollectionCard(summary, loader, thumbnail, { actions.openCollection(collection) }, { actions.more(OrganizeItem.Collection(collection)) })
                }
            }
        }
        val scrolled by remember { derivedStateOf { grid.canScrollBackward } }
        MemeDockTopBar(stringResource(R.string.tab_collections), glass, back = back, scrolled = scrolled) {
            IconButton(newCollection, Modifier.testTag("collections-new"), enabled = !state.loading && state.error == null) {
                Icon(MemeDockIcons.Add, stringResource(R.string.new_collection))
            }
        }
    }
}

private val OrganizeState.editingCollection get() = editing == OrganizeEdit.NewCollection || editing is OrganizeItem.Collection

/** Renders nothing unless a collection is being created or renamed. */
@Composable
internal fun CollectionEditor(state: OrganizeState, actions: OrganizeActions, modifier: Modifier = Modifier) {
    if (state.editingCollection) MemeDockInlineEdit(
        (state.editing as? OrganizeItem.Collection)?.name.orEmpty(), stringResource(R.string.name), actions.commit, { actions.edit(null) },
        modifier, busy = state.busy, error = state.editError?.let { failureText(it) })
}
