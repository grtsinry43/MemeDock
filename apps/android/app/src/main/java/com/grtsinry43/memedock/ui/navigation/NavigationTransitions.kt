package com.grtsinry43.memedock.ui.navigation

import androidx.compose.animation.*
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.navigation3.ui.NavDisplay
import com.grtsinry43.memedock.ui.components.LocalStickerTransition
import com.grtsinry43.memedock.ui.theme.MemeDockMotion

internal fun forwardPage(direction: Int): ContentTransform =
    slideInHorizontally(tween(MemeDockMotion.Page, easing = MemeDockMotion.Rebound)) { it * direction } togetherWith
        slideOutHorizontally(tween(MemeDockMotion.Page, easing = MemeDockMotion.Rebound)) { -it * direction / 3 }

internal fun backwardPage(direction: Int): ContentTransform =
    slideInHorizontally(tween(MemeDockMotion.Page, easing = MemeDockMotion.Rebound)) { -it * direction / 3 } togetherWith
        slideOutHorizontally(tween(MemeDockMotion.PageReturn, easing = MemeDockMotion.Rebound)) { it * direction }

internal fun tabTransition(direction: Int): ContentTransform =
    ContentTransform(
        targetContentEnter = slideInHorizontally(tween(MemeDockMotion.Tab, easing = MemeDockMotion.Rebound)) {
            it * direction / 2
        } + fadeIn(tween(MemeDockMotion.Tab, easing = MemeDockMotion.Smooth)),
        initialContentExit = slideOutHorizontally(tween(MemeDockMotion.Tab, easing = MemeDockMotion.Rebound)) {
            -it * direction / 2
        } + fadeOut(tween(MemeDockMotion.Tab, easing = MemeDockMotion.Smooth)),
        sizeTransform = SizeTransform(clip = false),
    )

// Images move through the shared overlay. Only the surrounding page fades here;
// its top and bottom elements move independently on the same navigation transition.
internal val stickerPageTransitions: Map<String, Any> =
    NavDisplay.transitionSpec {
        fadeIn(tween(MemeDockMotion.Page, easing = MemeDockMotion.Smooth)) togetherWith
            fadeOut(tween(MemeDockMotion.Fade, easing = MemeDockMotion.Smooth))
    } + NavDisplay.popTransitionSpec {
        fadeIn(tween(MemeDockMotion.Fade, easing = MemeDockMotion.Smooth)) togetherWith
            fadeOut(tween(MemeDockMotion.Page, easing = MemeDockMotion.Smooth))
    } + NavDisplay.predictivePopTransitionSpec { _ ->
        fadeIn(tween(MemeDockMotion.Fade, easing = MemeDockMotion.Smooth)) togetherWith
            fadeOut(tween(MemeDockMotion.Page, easing = MemeDockMotion.Smooth))
    }

@Composable
internal fun Modifier.detailElementTransition(fromTop: Boolean): Modifier {
    val scope = LocalStickerTransition.current?.visibility ?: return this
    val distance = with(LocalDensity.current) { 32.dp.roundToPx() }
    val direction = if (fromTop) -1 else 1
    return with(scope) {
        animateEnterExit(
            enter = slideInVertically(tween(MemeDockMotion.Page, easing = MemeDockMotion.Rebound)) {
                minOf(it, distance) * direction
            },
            exit = slideOutVertically(tween(MemeDockMotion.Page, easing = MemeDockMotion.Rebound)) {
                minOf(it, distance) * direction
            },
            label = if (fromTop) "detail-top" else "detail-bottom",
        )
    }
}
