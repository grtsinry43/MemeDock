package com.grtsinry43.memedock.feature.library

import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.library.ThumbnailState
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.components.MemeDockSkeleton
import com.grtsinry43.memedock.ui.components.stickerTransition
import java.io.File

/** Without [open] the tile takes no taps, leaving its gestures to [modifier] (for example a drag handle). */
@Composable
fun LibraryTile(item: LibraryItem, loader: ImageLoader, retry: () -> Unit, open: (() -> Unit)?, more: (() -> Unit)?,
    modifier: Modifier = Modifier) {
    var failed by remember(item.id, item.thumbnailPath) { mutableStateOf(false) }
    var attempt by remember(item.id) { mutableIntStateOf(0) }
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val request = remember(item.thumbnailPath, attempt) {
        item.thumbnailPath?.let { ImageRequest.Builder(context).data(File(it)).memoryCacheKey("${item.id}:$attempt").build() }
    }
    val shape = MaterialTheme.shapes.medium
    Box(
        modifier.testTag("sticker:${item.id}").stickerTransition(item.id).aspectRatio(1f).clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerLowest)
            .then(if (open == null) Modifier else Modifier.combinedClickable(
                onLongClickLabel = more?.let { stringResource(R.string.more_actions) },
                onLongClick = more?.let { { haptics.performHapticFeedback(HapticFeedbackType.LongPress); it() } },
                onClick = open,
            )),
        contentAlignment = Alignment.Center,
    ) {
        when {
            request != null && !failed -> AsyncImage(model = request, imageLoader = loader, contentDescription = item.title,
                contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize().padding(8.dp), onError = { failed = true })
            failed || item.thumbnailState == ThumbnailState.Failed -> IconButton(onClick = { failed = false; attempt++; retry() }) {
                Icon(MemeDockIcons.Image, stringResource(R.string.retry_thumbnail), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            else -> MemeDockSkeleton(Modifier.fillMaxSize(), shape)
        }
        if (item.animated) Text(stringResource(R.string.animated_image), style = MaterialTheme.typography.labelSmall,
            color = Color.White,
            modifier = Modifier.align(Alignment.BottomStart).padding(6.dp)
                .background(Color.Black.copy(alpha = .45f), MaterialTheme.shapes.extraSmall)
                .padding(horizontal = 6.dp, vertical = 2.dp))
        if (item.starred) Box(Modifier.align(Alignment.TopEnd).padding(6.dp).size(22.dp)
            .background(MaterialTheme.colorScheme.surfaceContainerLowest, CircleShape), contentAlignment = Alignment.Center) {
            Icon(MemeDockIcons.StarFilled, stringResource(R.string.favorite), Modifier.size(15.dp), tint = MaterialTheme.colorScheme.tertiary)
        }
    }
}
