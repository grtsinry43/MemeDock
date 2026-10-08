package com.grtsinry43.memedock.feature.organize

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import coil3.compose.AsyncImage
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.CollectionSummary
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.components.MemeDockSkeleton
import kotlinx.coroutines.CancellationException
import java.io.File

internal val CollectionCardWidth = 152.dp

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
            else Icon(MemeDockIcons.Folder, null, Modifier.size(40.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        CardCaption {
            Text(summary.collection.name, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(stringResource(R.string.collection_count, summary.count), style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
        }
    }
}

/** Ends the collection row the way the add chip ends the tags. */
@Composable
internal fun NewCollectionCard(onClick: () -> Unit, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Column(modifier.clip(MaterialTheme.shapes.large).background(colors.primary.copy(alpha = .12f))
        .clickable(role = Role.Button, onClick = onClick).testTag("organize-new-collection")) {
        Box(Modifier.fillMaxWidth().aspectRatio(1f), contentAlignment = Alignment.Center) {
            Icon(MemeDockIcons.Add, null, Modifier.size(40.dp), tint = colors.primary)
        }
        CardCaption {
            Text(stringResource(R.string.new_collection), style = MaterialTheme.typography.titleSmall, color = colors.primary,
                maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
internal fun CollectionCardSkeleton(modifier: Modifier = Modifier) {
    Column(modifier.clip(MaterialTheme.shapes.large).background(MaterialTheme.colorScheme.surfaceContainerLowest)) {
        MemeDockSkeleton(Modifier.fillMaxWidth().aspectRatio(1f), RectangleShape)
        CardCaption {
            MemeDockSkeleton(Modifier.fillMaxWidth(.7f).height(14.dp), MaterialTheme.shapes.extraSmall)
            MemeDockSkeleton(Modifier.fillMaxWidth(.4f).height(12.dp), MaterialTheme.shapes.extraSmall)
        }
    }
}

/** Fixed height so cards line up whichever script their names use. */
@Composable
private fun CardCaption(content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 12.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp, Alignment.CenterVertically), content = content)
}
