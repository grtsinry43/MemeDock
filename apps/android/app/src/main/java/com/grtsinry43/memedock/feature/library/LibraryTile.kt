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
import com.grtsinry43.memedock.R
import coil3.ImageLoader
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.library.ThumbnailState
import java.io.File

@Composable
fun LibraryTile(item: LibraryItem, loader: ImageLoader, retry: () -> Unit) {
    var failed by remember(item.id, item.thumbnailPath) { mutableStateOf(false) }
    var attempt by remember(item.id) { mutableIntStateOf(0) }
    val context = LocalContext.current
    val request = remember(item.thumbnailPath, attempt) { item.thumbnailPath?.let { ImageRequest.Builder(context).data(File(it)).memoryCacheKey("${item.id}:$attempt").build() } }
    Column {
        Surface(shape = MaterialTheme.shapes.medium, color = MaterialTheme.colorScheme.surfaceContainerLow) {
            Box(Modifier.fillMaxWidth().aspectRatio(1f), contentAlignment = Alignment.Center) {
                if (request != null && !failed) AsyncImage(model = request, imageLoader = loader, contentDescription = item.title,
                    contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize().padding(4.dp), onError = { failed = true })
                else if (failed || item.thumbnailState == ThumbnailState.Failed) TextButton(onClick = { failed = false; attempt++; retry() }) { Text(stringResource(R.string.retry_thumbnail)) }
                else CircularProgressIndicator(Modifier.size(24.dp))
                if (item.animated) Surface(modifier = Modifier.align(Alignment.BottomEnd).padding(4.dp), shape = MaterialTheme.shapes.small, color = MaterialTheme.colorScheme.secondaryContainer) {
                    Text(stringResource(if (item.mime == "image/png") R.string.apng_first_frame else R.string.animated_image), style = MaterialTheme.typography.labelSmall, modifier = Modifier.padding(4.dp))
                }
            }
        }
        Text(item.title, maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(top = 4.dp))
    }
}
