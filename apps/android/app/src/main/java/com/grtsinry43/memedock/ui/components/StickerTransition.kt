package com.grtsinry43.memedock.ui.components

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.EnterExitState
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.core.tween
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.grtsinry43.memedock.ui.theme.MemeDockMotion

class StickerTransition(val shared: SharedTransitionScope, val visibility: AnimatedVisibilityScope,
    val pageActive: Boolean)
val LocalStickerTransition = compositionLocalOf<StickerTransition?> { null }

@Composable
fun ProvideStickerTransition(shared: SharedTransitionScope, content: @Composable () -> Unit) {
    val visibility = LocalNavAnimatedContentScope.current
    val active = visibility.transition.targetState == EnterExitState.Visible
    CompositionLocalProvider(LocalStickerTransition provides StickerTransition(shared, visibility, active), content = content)
}

@Composable
fun Modifier.stickerTransition(id: String): Modifier {
    val scope = LocalStickerTransition.current ?: return this
    return with(scope.shared) {
        sharedElement(rememberSharedContentState("sticker:$id"), scope.visibility,
            boundsTransform = { _, _ -> tween(MemeDockMotion.SharedImage, easing = MemeDockMotion.Rebound) })
    }
}
