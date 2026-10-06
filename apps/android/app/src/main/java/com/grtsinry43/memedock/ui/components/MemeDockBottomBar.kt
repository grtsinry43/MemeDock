package com.grtsinry43.memedock.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import com.grtsinry43.memedock.ui.theme.MemeDockMotion
import dev.chrisbanes.haze.HazeState

enum class HomeTab { Stickers, Organize, Mine }

@Composable
fun MemeDockBottomBar(selected: HomeTab, select: (HomeTab) -> Unit, glass: HazeState, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth().glass(glass, glassStyle())) {
        HorizontalDivider(thickness = MemeDockLayout.Hairline, color = MaterialTheme.colorScheme.outlineVariant)
        Row(
            Modifier.windowInsetsPadding(WindowInsets.navigationBars).fillMaxWidth()
                .height(MemeDockLayout.BottomBarHeight).selectableGroup(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            HomeTab.entries.forEach { tab ->
                val active = selected == tab
                val color by animateColorAsState(
                    if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    tween(MemeDockMotion.Feedback), label = "tab-color",
                )
                Column(
                    Modifier.weight(1f).fillMaxHeight().testTag("tab:${tab.name}")
                        .selectable(active, interactionSource = null, indication = ripple(bounded = false, radius = 36.dp),
                            role = Role.Tab, onClick = { select(tab) }),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(2.dp, Alignment.CenterVertically),
                ) {
                    Icon(when (tab) {
                        HomeTab.Stickers -> MemeDockIcons.Mood
                        HomeTab.Organize -> MemeDockIcons.Collections
                        HomeTab.Mine -> MemeDockIcons.Person
                    }, null, Modifier.size(24.dp), tint = color)
                    Text(stringResource(when (tab) {
                        HomeTab.Stickers -> R.string.tab_stickers
                        HomeTab.Organize -> R.string.tab_organize
                        HomeTab.Mine -> R.string.tab_mine
                    }), style = MaterialTheme.typography.labelSmall, color = color)
                }
            }
        }
    }
}
