package com.grtsinry43.memedock.ui.components

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R

enum class HomeTab { Stickers, Search, Collections, Settings }

@Composable
fun MemeDockBottomBar(selected: HomeTab, select: (HomeTab) -> Unit) {
    Column {
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = .35f))
        NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainer, tonalElevation = 0.dp) {
            HomeTab.entries.forEach { tab ->
                val label = stringResource(when (tab) {
                    HomeTab.Stickers -> R.string.tab_stickers
                    HomeTab.Search -> R.string.tab_search
                    HomeTab.Collections -> R.string.tab_collections
                    HomeTab.Settings -> R.string.tab_settings
                })
                NavigationBarItem(selected = selected == tab, onClick = { select(tab) },
                    modifier = Modifier.testTag("tab:${tab.name}"),
                    icon = { Icon(when (tab) {
                        HomeTab.Stickers -> MemeDockIcons.Sticker
                        HomeTab.Search -> MemeDockIcons.Search
                        HomeTab.Collections -> MemeDockIcons.Collections
                        HomeTab.Settings -> MemeDockIcons.Settings
                    }, null, Modifier.size(24.dp)) },
                    label = { Text(label, style = MaterialTheme.typography.labelSmall) })
            }
        }
    }
}
