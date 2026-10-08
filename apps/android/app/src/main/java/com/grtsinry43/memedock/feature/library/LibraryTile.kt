package com.grtsinry43.memedock.feature.library

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.library.ThumbnailState
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.components.MemeDockSelectionMark
import com.grtsinry43.memedock.ui.components.MemeDockSkeleton
import com.grtsinry43.memedock.ui.components.stickerTransition
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import com.grtsinry43.memedock.ui.theme.MemeDockMotion
import java.io.File

/**
 * Stickers sit bare on the page; the rounded highlight appears only under a press, a selection or a drag ([lifted]).
 * Without [open] the tile takes no taps, leaving its gestures to [modifier] (for example a drag handle).
 */
@Composable
fun LibraryTile(item: LibraryItem, loader: ImageLoader, retry: () -> Unit, open: (() -> Unit)?, more: (() -> Unit)?,
    modifier: Modifier = Modifier, selected: Boolean = false, selecting: Boolean = false, lifted: Boolean = false) {
    var failed by remember(item.id, item.thumbnailPath) { mutableStateOf(false) }
    var attempt by remember(item.id) { mutableIntStateOf(0) }
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    val request = remember(item.thumbnailPath, attempt) {
        item.thumbnailPath?.let { ImageRequest.Builder(context).data(File(it)).memoryCacheKey("${item.id}:$attempt").build() }
    }
    val shape = MaterialTheme.shapes.small
    val scale by animateFloatAsState(if (selected) .9f else 1f, tween(MemeDockMotion.Feedback), label = "tile-scale")
    val highlight by animateColorAsState(when {
        selected -> colors.primary.copy(alpha = .12f)
        lifted -> colors.surfaceContainerLowest
        else -> colors.surfaceContainerLowest.copy(alpha = 0f)
    }, tween(MemeDockMotion.Feedback), label = "tile-highlight")
    val elevation by animateDpAsState(if (lifted) 6.dp else 0.dp, tween(MemeDockMotion.Feedback), label = "tile-elevation")
    Box(
        modifier.testTag("sticker:${item.id}").stickerTransition(item.id).aspectRatio(1f)
            .shadow(elevation, shape).clip(shape).background(highlight)
            .then(if (selecting) Modifier.semantics { this.selected = selected } else Modifier)
            .then(if (open == null) Modifier else Modifier.combinedClickable(
                role = if (selecting) Role.Checkbox else Role.Button,
                onLongClickLabel = more?.let { stringResource(R.string.more_actions) },
                onLongClick = more?.let { { haptics.performHapticFeedback(HapticFeedbackType.LongPress); it() } },
                onClick = open,
            ))
            .padding(MemeDockLayout.GapXSmall),
    ) {
        Box(Modifier.fillMaxSize().graphicsLayer { scaleX = scale; scaleY = scale }, contentAlignment = Alignment.Center) {
            when {
                request != null && !failed -> AsyncImage(model = request, imageLoader = loader, contentDescription = item.title,
                    contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize(), onError = { failed = true })
                failed || item.thumbnailState == ThumbnailState.Failed -> IconButton(onClick = { failed = false; attempt++; retry() }) {
                    Icon(MemeDockIcons.Image, stringResource(R.string.retry_thumbnail), tint = colors.onSurfaceVariant)
                }
                else -> MemeDockSkeleton(Modifier.fillMaxSize(), shape)
            }
            if (item.animated) Text(stringResource(R.string.animated_image), style = MaterialTheme.typography.labelSmall,
                color = Color.White,
                modifier = Modifier.align(Alignment.BottomStart)
                    .background(Color.Black.copy(alpha = .45f), MaterialTheme.shapes.extraSmall)
                    .padding(horizontal = 6.dp, vertical = 2.dp))
        }
        if (selecting) MemeDockSelectionMark(selected, Modifier.align(Alignment.TopEnd),
            stringResource(if (selected) R.string.selected else R.string.not_selected))
        else if (item.starred) Box(Modifier.align(Alignment.TopEnd).size(MemeDockLayout.IconLarge)
            .background(colors.surfaceContainerLowest, CircleShape), contentAlignment = Alignment.Center) {
            Icon(MemeDockIcons.StarFilled, stringResource(R.string.favorite), Modifier.size(MemeDockLayout.IconSmall), tint = colors.tertiary)
        }
    }
}
