package com.grtsinry43.memedock.ui.components

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.core.tween
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import com.grtsinry43.memedock.ui.theme.MemeDockMotion

class StickerTransition(val shared: SharedTransitionScope, val visibility: AnimatedVisibilityScope,
    val pageActive: Boolean)
val LocalStickerTransition = compositionLocalOf<StickerTransition?> { null }

@Composable
fun Modifier.stickerTransition(id: String): Modifier {
    val scope = LocalStickerTransition.current ?: return this
    return with(scope.shared) {
        sharedElement(rememberSharedContentState("sticker:$id"), scope.visibility,
            boundsTransform = { _, _ -> tween(MemeDockMotion.SharedImage) })
    }
}
