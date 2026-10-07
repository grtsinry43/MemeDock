package com.grtsinry43.memedock.feature.library

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.*
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.HazeTint

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
    val shape = MaterialTheme.shapes.small
    Row(
        Modifier.widthIn(max = 560.dp).fillMaxWidth().height(44.dp).shadow(1.dp, shape).clip(shape)
            .glass(glass, glassStyle().copy(tints = listOf(HazeTint(colors.background.copy(alpha = .88f)))))
            .border(1.dp, colors.outlineVariant.copy(alpha = .5f), shape)
            .padding(start = 16.dp, end = 4.dp).testTag("selection-bar"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(stringResource(if (state.selected.isEmpty()) R.string.selection_prompt else R.string.selection_count, state.selected.size),
            Modifier.weight(1f).semantics { liveRegion = LiveRegionMode.Polite },
            style = MaterialTheme.typography.bodyMedium,
            color = if (state.selected.isEmpty()) colors.onSurfaceVariant else colors.onSurface)
        TextButton(onClick = organize, enabled = state.selected.isNotEmpty() && !state.busy,
            modifier = Modifier.height(44.dp).testTag("selection-organize")) {
            Text(stringResource(R.string.organize))
        }
        TextButton(onClick = exit, enabled = !state.busy, modifier = Modifier.height(44.dp).testTag("selection-close")) {
            Text(stringResource(R.string.done))
        }
    }
}
