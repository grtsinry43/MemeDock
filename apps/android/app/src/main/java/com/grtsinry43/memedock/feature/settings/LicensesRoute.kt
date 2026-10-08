package com.grtsinry43.memedock.feature.settings

import android.content.res.Resources
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import com.mikepenz.aboutlibraries.Libs
import com.mikepenz.aboutlibraries.entity.Library
import dev.chrisbanes.haze.rememberHazeState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private sealed interface LicenseList {
    data object Loading : LicenseList
    data object Failed : LicenseList
    data class Ready(val libraries: List<Library>) : LicenseList {
        /** Multiplatform redistributions reuse the AndroidX names, so these rows also show their group. */
        val sharedNames = libraries.groupingBy { it.name }.eachCount().filterValues { it > 1 }.keys
    }
}

@Composable
fun LicensesRoute(back: () -> Unit) {
    val resources = LocalResources.current
    val list by produceState<LicenseList>(LicenseList.Loading, resources) {
        value = try { LicenseList.Ready(withContext(Dispatchers.IO) { resources.libraries() }) }
        catch (cancel: CancellationException) { throw cancel }
        catch (_: Exception) { LicenseList.Failed }
    }
    val glass = rememberHazeState()
    val listState = rememberLazyListState()
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + MemeDockLayout.TopBarHeight
    val bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding() + MemeDockLayout.SectionGap
    val uris = LocalUriHandler.current
    val messages = LocalMemeDockMessages.current
    val scope = rememberCoroutineScope()
    val openFailed = stringResource(R.string.licenses_open_failed)
    val open: (String) -> Unit = { uri ->
        try { uris.openUri(uri) }
        catch (_: IllegalArgumentException) {
            scope.launch { messages.showMemeDockMessage(MemeDockMessage(openFailed, MemeDockMessageType.Error)) }
        }
    }
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)
        .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))) {
        LazyColumn(Modifier.fillMaxSize().glassSource(glass), listState,
            PaddingValues(start = MemeDockLayout.PagePadding, end = MemeDockLayout.PagePadding, top = top + MemeDockLayout.GapSmall, bottom = bottom)) {
            when (val current = list) {
                LicenseList.Loading -> Unit
                LicenseList.Failed -> item(key = "failed") {
                    Box(Modifier.fillMaxWidth().padding(top = 48.dp), contentAlignment = Alignment.Center) {
                        MemeDockEmptyState(stringResource(R.string.licenses_load_failed), failureText(null), icon = MemeDockIcons.Alert)
                    }
                }
                is LicenseList.Ready -> itemsIndexed(current.libraries, key = { _, library -> library.uniqueId }) { index, library ->
                    LicenseRow(library, library.name in current.sharedNames, index == 0, index == current.libraries.lastIndex, open)
                }
            }
        }
        val scrolled by remember { derivedStateOf { listState.canScrollBackward } }
        MemeDockTopBar(stringResource(R.string.settings_licenses), glass, back = back, scrolled = scrolled)
    }
}

/** One row of a group that is too long to compose eagerly; the ends carry the group's corners. */
@Composable
private fun LicenseRow(library: Library, sharedName: Boolean, first: Boolean, last: Boolean, open: (String) -> Unit) {
    val corners = MaterialTheme.shapes.large
    val shape = corners.copy(
        topStart = if (first) corners.topStart else CornerSize(0), topEnd = if (first) corners.topEnd else CornerSize(0),
        bottomStart = if (last) corners.bottomStart else CornerSize(0), bottomEnd = if (last) corners.bottomEnd else CornerSize(0))
    val link = library.licenses.firstNotNullOfOrNull { it.url } ?: library.website ?: library.scm?.url
    val details = listOf(library.uniqueId.substringBefore(':').takeIf { sharedName }, library.artifactVersion,
        library.licenses.joinToString { it.name }).filterNot { it.isNullOrBlank() }
    Surface(shape = shape, color = MaterialTheme.colorScheme.surfaceContainerLowest) {
        Column {
            if (!first) HorizontalDivider(Modifier.padding(start = 16.dp), MemeDockLayout.Hairline, MaterialTheme.colorScheme.outlineVariant)
            MemeDockRow(library.name, subtitle = details.joinToString(" · ").ifEmpty { null },
                onClick = link?.let { { open(it) } },
                trailing = link?.let { { Icon(MemeDockIcons.OpenInNew, null, Modifier.size(MemeDockLayout.IconMedium), tint = MaterialTheme.colorScheme.outline) } })
        }
    }
}

private fun Resources.libraries(): List<Library> {
    val json = openRawResource(R.raw.aboutlibraries).bufferedReader().use { it.readText() }
    return Libs.Builder().withJson(json).build().libraries
}
