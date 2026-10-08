package com.grtsinry43.memedock.feature.library

import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.semantics
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.*
import dev.chrisbanes.haze.HazeState

@Composable
internal fun SelectionModeButton(actions: StickerGridActions) {
    val selecting = actions.selection?.selecting == true
    if (actions.startSelection != null) IconButton(
        onClick = { if (selecting) actions.exitSelection?.invoke() else actions.startSelection.invoke() },
        enabled = actions.selection?.busy != true,
        modifier = Modifier.testTag("library-select"),
    ) {
        Icon(MemeDockIcons.Select, stringResource(if (selecting) R.string.finish_selection else R.string.select_items),
            tint = if (selecting) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
internal fun SelectionBar(state: BatchSelectionState, glass: HazeState, organize: () -> Unit, exit: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    MemeDockFloatingBar(glass, Modifier.testTag("selection-bar")) {
        Text(stringResource(if (state.selected.isEmpty()) R.string.selection_prompt else R.string.selection_count, state.selected.size),
            Modifier.weight(1f).semantics { liveRegion = LiveRegionMode.Polite },
            style = MaterialTheme.typography.bodyMedium,
            color = if (state.selected.isEmpty()) colors.onSurfaceVariant else colors.onSurface)
        TextButton(onClick = organize, enabled = state.selected.isNotEmpty() && !state.busy,
            modifier = Modifier.testTag("selection-organize")) {
            Text(stringResource(R.string.organize))
        }
        TextButton(onClick = exit, enabled = !state.busy, modifier = Modifier.testTag("selection-close")) {
            Text(stringResource(R.string.done))
        }
    }
}
