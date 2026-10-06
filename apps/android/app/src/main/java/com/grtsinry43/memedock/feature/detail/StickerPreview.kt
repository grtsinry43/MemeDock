package com.grtsinry43.memedock.feature.detail

import android.graphics.drawable.Animatable
import androidx.compose.foundation.Image
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import coil3.ImageLoader
import coil3.DrawableImage
import coil3.compose.AsyncImage
import coil3.compose.asPainter
import coil3.request.*
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.StickerDetails
import com.grtsinry43.memedock.ui.components.LocalStickerTransition
import com.grtsinry43.memedock.ui.components.stickerTransition
import java.io.File

@Composable
fun StickerPreview(detail: StickerDetails, loader: ImageLoader, playing: Boolean, initialThumbnail: String? = null) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val transition = LocalStickerTransition.current
    val moving = transition?.shared?.isTransitionActive == true
    val active = transition?.pageActive != false
    var image by remember(detail.id, detail.previewPath) { mutableStateOf<coil3.Image?>(null) }
    var animation by remember(detail.id, detail.previewPath) { mutableStateOf<Animatable?>(null) }
    var failed by remember(detail.id, detail.previewPath) { mutableStateOf(false) }
    var attempt by remember(detail.id) { mutableIntStateOf(0) }
    var scale by remember(detail.id) { mutableFloatStateOf(1f) }
    var offset by remember(detail.id) { mutableStateOf(Offset.Zero) }
    val windowSize = LocalWindowInfo.current.containerSize
    val windowHeight = with(LocalDensity.current) { windowSize.height.toDp() }
    val height = if (windowHeight < 480.dp) 200.dp else 360.dp
    val thumbnail = remember(initialThumbnail) { initialThumbnail?.let { ImageRequest.Builder(context).data(File(it)).size(512).build() } }
    LaunchedEffect(detail.id, detail.previewPath, attempt) {
        val path = detail.previewPath ?: return@LaunchedEffect
        failed = false
        val result = loader.execute(ImageRequest.Builder(context).data(File(path))
            .size(1024, 1024).memoryCachePolicy(CachePolicy.DISABLED).diskCachePolicy(CachePolicy.DISABLED).build())
        if (result is SuccessResult) {
            animation = (result.image as? DrawableImage)?.drawable as? Animatable
            animation?.stop()
            image = result.image
        } else failed = true
    }
    // DrawablePainter is a RememberObserver. Register it in composition so
    // invalidation and animation callbacks are attached and released correctly.
    val painter = remember(image, context) { image?.asPainter(context) }
    DisposableEffect(animation, playing, lifecycle, active, moving, painter) {
        fun update() {
            animation?.let { if (playing && active && !moving && lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) it.start() else it.stop() }
        }
        val observer = LifecycleEventObserver { _, _ -> update() }
        lifecycle.addObserver(observer)
        update()
        onDispose { lifecycle.removeObserver(observer); animation?.stop() }
    }
    Surface(modifier = Modifier.fillMaxWidth().testTag("detail-preview").stickerTransition(detail.id), color = MaterialTheme.colorScheme.surfaceContainerLowest,
        shape = MaterialTheme.shapes.large) {
        Box(Modifier.fillMaxWidth().height(height).clipToBounds().pointerInput(detail.id, moving) {
            if (!moving) detectTransformGestures { _, pan, zoom, _ ->
                scale = (scale * zoom).coerceIn(1f, 4f)
                val bound = size.width * (scale - 1f) / 2f
                val yBound = size.height * (scale - 1f) / 2f
                offset = if (scale == 1f) Offset.Zero else Offset((offset.x + pan.x).coerceIn(-bound, bound), (offset.y + pan.y).coerceIn(-yBound, yBound))
            }
        }, contentAlignment = androidx.compose.ui.Alignment.Center) {
            val imageModifier = Modifier.fillMaxSize().padding(12.dp).graphicsLayer {
                scaleX = if (moving) 1f else scale; scaleY = if (moving) 1f else scale
                translationX = if (moving) 0f else offset.x; translationY = if (moving) 0f else offset.y
            }
            val loaded = painter
            if (loaded != null && !moving) Image(loaded, detail.title, imageModifier, contentScale = ContentScale.Fit)
            else if (thumbnail != null) AsyncImage(thumbnail, detail.title, loader, modifier = imageModifier, contentScale = ContentScale.Fit)
            if (failed && !moving) TextButton(onClick = { attempt++ }) { Text(stringResource(R.string.preview_retry)) }
            else if (loaded == null && thumbnail == null) CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 3.dp)
        }
    }
}
