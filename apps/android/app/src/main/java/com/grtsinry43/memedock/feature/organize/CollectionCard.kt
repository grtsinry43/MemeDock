package com.grtsinry43.memedock.feature.organize

import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import coil3.compose.AsyncImage
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.CollectionSummary
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import kotlinx.coroutines.CancellationException
import java.io.File

@Composable
internal fun CollectionCard(summary: CollectionSummary, loader: ImageLoader, thumbnail: suspend (String) -> String,
    open: () -> Unit, more: () -> Unit, modifier: Modifier = Modifier) {
    var path by remember(summary.coverId, summary.thumbnailPath) { mutableStateOf(summary.thumbnailPath) }
    LaunchedEffect(summary.coverId, summary.thumbnailPath) {
        val id = summary.coverId
        if (id != null && path == null) try { path = thumbnail(id) }
        catch (cancel: CancellationException) { throw cancel }
        catch (_: Exception) { path = null }
    }
    Column(modifier.clip(MaterialTheme.shapes.large)
        .background(MaterialTheme.colorScheme.surfaceContainerLowest)
        .combinedClickable(onClick = open, onLongClick = more, onLongClickLabel = stringResource(R.string.more_actions))) {
        Box(Modifier.fillMaxWidth().aspectRatio(1f).background(MaterialTheme.colorScheme.surfaceContainer), contentAlignment = Alignment.Center) {
            val imagePath = path
            if (imagePath != null) AsyncImage(File(imagePath), summary.collection.name, imageLoader = loader,
                contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize().padding(12.dp), onError = { path = null })
            else Icon(MemeDockIcons.Collections, null, Modifier.size(40.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(summary.collection.name, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(stringResource(R.string.collection_count, summary.count), style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
