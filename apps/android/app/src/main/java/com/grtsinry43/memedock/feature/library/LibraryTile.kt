package com.grtsinry43.memedock.feature.library

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.platform.testTag
import com.grtsinry43.memedock.ui.components.stickerTransition
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.R
import coil3.ImageLoader
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.library.ThumbnailState
import java.io.File

@Composable
fun LibraryTile(item: LibraryItem, loader: ImageLoader, retry: () -> Unit, open: () -> Unit) {
    var failed by remember(item.id, item.thumbnailPath) { mutableStateOf(false) }
    var attempt by remember(item.id) { mutableIntStateOf(0) }
    val context = LocalContext.current
    val request = remember(item.thumbnailPath, attempt) { item.thumbnailPath?.let { ImageRequest.Builder(context).data(File(it)).memoryCacheKey("${item.id}:$attempt").build() } }
    Column {
        Surface(onClick = open, modifier = Modifier.testTag("sticker:${item.id}").stickerTransition(item.id),
            shape = androidx.compose.ui.graphics.RectangleShape, color = MaterialTheme.colorScheme.surfaceContainerLow) {
            Box(Modifier.fillMaxWidth().aspectRatio(1f), contentAlignment = Alignment.Center) {
                if (request != null && !failed) AsyncImage(model = request, imageLoader = loader, contentDescription = item.title,
                    contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize().padding(4.dp), onError = { failed = true })
                else if (failed || item.thumbnailState == ThumbnailState.Failed) TextButton(onClick = { failed = false; attempt++; retry() }) { Text(stringResource(R.string.retry_thumbnail)) }
                else CircularProgressIndicator(Modifier.size(24.dp))
                if (item.animated) Surface(modifier = Modifier.align(Alignment.BottomEnd).padding(8.dp), shape = MaterialTheme.shapes.small, color = MaterialTheme.colorScheme.secondaryContainer) {
                    Text(stringResource(R.string.animated_image), style = MaterialTheme.typography.labelSmall, modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp))
                }
                if (item.starred) Surface(modifier = Modifier.align(Alignment.TopEnd).padding(8.dp),
                    shape = MaterialTheme.shapes.small, color = MaterialTheme.colorScheme.primaryContainer) {
                    Icon(MemeDockIcons.Star, stringResource(R.string.favorite), Modifier.padding(5.dp).size(16.dp),
                        tint = MaterialTheme.colorScheme.onPrimaryContainer)
                }
            }
        }
        Text(item.title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelMedium, modifier = Modifier.padding(horizontal = 8.dp, vertical = 8.dp))
    }
}
