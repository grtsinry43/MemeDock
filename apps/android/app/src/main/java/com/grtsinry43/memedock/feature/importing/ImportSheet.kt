package com.grtsinry43.memedock.feature.importing

import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import coil3.compose.AsyncImage
import coil3.decode.BitmapFactoryDecoder
import coil3.request.ImageRequest
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryCollection
import com.grtsinry43.memedock.data.library.ManagementRepository
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureCode
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.sync.Semaphore
import java.io.File

private enum class ImportSheetMode { Review, Problems }

// Share admission across preview requests; animated originals need only a static first frame here.
private val importPreviewDecoder = BitmapFactoryDecoder.Factory(parallelismLock = Semaphore(2))

/**
 * Reviews a batch shared from another app, or lists what a finished batch could not add. Running batches never
 * open a sheet; their progress stays on the home page.
 */
@Composable
fun ImportSheet(state: ImportState, coordinator: ImportCoordinator, repository: ManagementRepository, loader: ImageLoader) {
    val mode = when {
        !state.visible -> null
        state.phase == ImportPhase.Preparing || state.phase == ImportPhase.Review -> ImportSheetMode.Review
        state.phase == ImportPhase.Finished && state.unresolved > 0 -> ImportSheetMode.Problems
        else -> null
    }
    // Keep the last visible content while the sheet animates away, instead of the state that closed it.
    val shownMode = rememberRetained(mode) ?: return
    val shown = rememberRetained(state.takeIf { mode != null }) ?: return
    when (shownMode) {
        ImportSheetMode.Review -> MemeDockSheet(
            visible = mode == ImportSheetMode.Review,
            onDismissRequest = coordinator::hide,
            title = stringResource(R.string.import_review_title, shown.items.size),
            subtitle = stringResource(R.string.import_review_hint),
        ) { ReviewContent(shown, coordinator, repository, loader) }
        ImportSheetMode.Problems -> MemeDockSheet(
            visible = mode == ImportSheetMode.Problems,
            onDismissRequest = coordinator::hide,
            title = stringResource(R.string.import_problems_title, shown.unresolved),
            subtitle = (shown.count(ImportItemStatus.Created) + shown.count(ImportItemStatus.Reused)).takeIf { it > 0 }
                ?.let { stringResource(R.string.import_problems_hint, it) },
        ) { ProblemsContent(shown, coordinator, loader) }
    }
}

@Composable
private fun MemeDockSheetScope.ReviewContent(state: ImportState, coordinator: ImportCoordinator,
    repository: ManagementRepository, loader: ImageLoader) {
    var collections by remember { mutableStateOf<List<LibraryCollection>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    LaunchedEffect(repository, refresh) {
        error = null
        try { collections = repository.collections(false) }
        catch (cancel: CancellationException) { throw cancel }
        catch (failure: Exception) { error = failureCode(failure) }
    }
    val preparing = state.phase == ImportPhase.Preparing
    LazyVerticalGrid(
        GridCells.Adaptive(72.dp),
        Modifier.fillMaxWidth().heightIn(max = 236.dp).padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        items(state.items, key = { it.candidate.uri }) { item ->
            ImportThumbnail(item, loader, item.stagedPath?.let(::File), Modifier.aspectRatio(1f))
        }
    }
    val unreadable = state.count(ImportItemStatus.Failed)
    if (unreadable > 0) Text(stringResource(R.string.import_unreadable, unreadable), style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(horizontal = 24.dp).padding(top = 8.dp))
    Text(stringResource(R.string.import_collection), style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp, bottom = 8.dp).semantics { heading() })
    val available = collections
    if (available != null) LazyRow(contentPadding = PaddingValues(horizontal = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        item(key = "none") {
            CollectionChoice(stringResource(R.string.no_collection), state.collection == null) { coordinator.selectCollection(null) }
        }
        items(available, key = { it.id }) { collection ->
            CollectionChoice(collection.name, state.collection?.id == collection.id) { coordinator.selectCollection(collection) }
        }
    }
    else if (error != null) Row(Modifier.fillMaxWidth().padding(start = 24.dp, end = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(failureText(error), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
        TextButton(onClick = { refresh++ }) { Text(stringResource(R.string.retry)) }
    }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(top = 20.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        val colors = MaterialTheme.colorScheme
        if (preparing || state.ready > 0) Button(
            onClick = { if (!preparing) { coordinator.start(); coordinator.hide() } },
            modifier = Modifier.fillMaxWidth().height(52.dp).testTag("import-start"),
            shape = MaterialTheme.shapes.medium,
        ) {
            if (preparing) {
                CircularProgressIndicator(Modifier.size(18.dp), color = colors.onPrimary, strokeWidth = 2.dp)
                Spacer(Modifier.width(10.dp))
                Text(stringResource(R.string.import_reading_files))
            } else Text(stringResource(R.string.import_add_count, state.ready))
        }
        TextButton(onClick = coordinator::discard, modifier = Modifier.fillMaxWidth().height(48.dp), shape = MaterialTheme.shapes.medium) {
            Text(stringResource(R.string.import_discard), color = colors.onSurfaceVariant)
        }
    }
}

@Composable
private fun MemeDockSheetScope.ProblemsContent(state: ImportState, coordinator: ImportCoordinator, loader: ImageLoader) {
    val sheet = this
    val problems = state.items.filter { it.status in ImportState.Unresolved }
    LazyColumn(Modifier.fillMaxWidth().heightIn(max = 360.dp)) {
        items(problems, key = { it.candidate.uri }) { item ->
            Row(Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(horizontal = 24.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                // After a run the staged copy is gone, so the preview reads the source; unreadable sources show a placeholder.
                ImportThumbnail(item, loader, item.stagedPath?.let(::File) ?: Uri.parse(item.candidate.uri), Modifier.size(52.dp))
                Text(problemText(item), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium,
                    maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
    }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (state.retryable) Button(onClick = { coordinator.retryFailed(); coordinator.hide() },
            modifier = Modifier.fillMaxWidth().height(52.dp).testTag("import-retry"), shape = MaterialTheme.shapes.medium) {
            Text(stringResource(R.string.import_retry_failed))
        }
        TextButton(onClick = { sheet.dismiss() }, modifier = Modifier.fillMaxWidth().height(48.dp), shape = MaterialTheme.shapes.medium) {
            Text(stringResource(R.string.done), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun problemText(item: ImportItemState) = when (item.status) {
    ImportItemStatus.RestoreRequired -> stringResource(R.string.import_restore_required)
    ImportItemStatus.Cancelled -> stringResource(R.string.import_cancelled)
    else -> failureText(item.error ?: "INTERNAL")
}

/** Review passes only the staged copy as [source], so a shared URI is read once; null shows a placeholder. */
@Composable
private fun ImportThumbnail(item: ImportItemState, loader: ImageLoader, source: Any?, modifier: Modifier) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val working = item.status == ImportItemStatus.Queued || item.status == ImportItemStatus.Reading
    val bad = item.status == ImportItemStatus.Failed || item.status == ImportItemStatus.RestoreRequired
    Box(modifier.clip(MaterialTheme.shapes.small).background(colors.surfaceContainer), contentAlignment = Alignment.Center) {
        val request = remember(context, source) {
            source?.let {
                ImageRequest.Builder(context)
                    .data(it)
                    .size(192)
                    .decoderFactory(importPreviewDecoder)
                    .memoryCacheKeyExtra("memedock:import-preview", "static-first-frame")
                    .build()
            }
        }
        if (request != null) AsyncImage(request, null, loader, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        else Icon(MemeDockIcons.Image, null, Modifier.size(20.dp), tint = colors.onSurfaceVariant)
        if (working && source == null) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
        if (bad) Box(Modifier.align(Alignment.BottomEnd).padding(4.dp).size(18.dp).background(colors.error, CircleShape),
            contentAlignment = Alignment.Center) {
            Icon(MemeDockIcons.Alert, null, Modifier.size(12.dp), tint = colors.onError)
        }
    }
}

@Composable
private fun CollectionChoice(name: String, selected: Boolean, select: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.heightIn(min = 36.dp).clip(MaterialTheme.shapes.small)
            .background(if (selected) colors.primary.copy(alpha = .12f) else colors.surfaceContainer)
            .clickable(role = Role.RadioButton, onClick = select).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (selected) Icon(MemeDockIcons.Check, null, Modifier.size(16.dp), tint = colors.primary)
        Text(name, style = MaterialTheme.typography.bodyMedium, color = if (selected) colors.primary else colors.onSurface,
            maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}
