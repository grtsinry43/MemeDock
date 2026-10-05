package com.grtsinry43.memedock.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.*
import androidx.compose.animation.core.tween
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.settings.ThemeMode
import com.grtsinry43.memedock.feature.collections.CollectionsRoute
import com.grtsinry43.memedock.feature.detail.DetailRoute
import com.grtsinry43.memedock.feature.importing.ImportSheet
import com.grtsinry43.memedock.feature.library.LibraryRoute
import com.grtsinry43.memedock.feature.search.SearchRoute
import com.grtsinry43.memedock.feature.settings.SettingsRoute
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.theme.*
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.retryWhen
import kotlinx.coroutines.delay
import java.io.IOException

private data class AppPage(val tab: HomeTab, val sticker: String?, val collection: String?, val collectionName: String?) {
    val stateKey get() = "${tab.name}:${collection.orEmpty()}"
}

@Composable
fun MemeDockApp(container: AppContainer) {
    var tab by rememberSaveable { mutableStateOf(HomeTab.Stickers) }
    var selected by rememberSaveable { mutableStateOf<String?>(null) }
    var collection by rememberSaveable { mutableStateOf<String?>(null) }
    var collectionName by rememberSaveable { mutableStateOf<String?>(null) }
    var initialItem by remember { mutableStateOf<LibraryItem?>(null) }
    val holder = rememberSaveableStateHolder()
    var previousCollection by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(collection) {
        previousCollection?.takeIf { it != collection }?.let { holder.removeState("Collections:$it") }
        previousCollection = collection
    }
    val keyboard = LocalSoftwareKeyboardController.current
    val focus = LocalFocusManager.current
    val imports by container.imports.state.collectAsStateWithLifecycle()
    val appearance = remember(container) { container.appearance.mode.retryWhen { error, _ ->
        if (error is IOException) { delay(1_000); true } else false
    }.catch { emit(ThemeMode.System) } }
    val mode by appearance.collectAsStateWithLifecycle(ThemeMode.System)
    val dark = when (mode) {
        ThemeMode.System -> isSystemInDarkTheme()
        ThemeMode.Light -> false
        ThemeMode.Dark -> true
    }
    val context = LocalContext.current
    val view = LocalView.current
    LaunchedEffect(dark, context, view) {
        context.activityWindow()?.let { window ->
            WindowCompat.getInsetsController(window, view).apply {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
    }
    val page = AppPage(tab, selected, collection, collectionName)
    val back = {
        when {
            selected != null -> selected = null
            collection != null -> { collection = null; collectionName = null }
            else -> tab = HomeTab.Stickers
        }
    }
    BackHandler(enabled = selected != null || collection != null || tab != HomeTab.Stickers, onBack = back)
    val open: (LibraryItem) -> Unit = { item ->
        focus.clearFocus(); keyboard?.hide(); initialItem = item; selected = item.id
    }
    MemeDockTheme(darkTheme = dark) {
        SharedTransitionLayout {
            AnimatedContent(page, modifier = Modifier.fillMaxSize(), transitionSpec = {
                (fadeIn(tween(MemeDockMotion.Page)) + slideInVertically(tween(MemeDockMotion.Page)) { it / 24 })
                    .togetherWith(fadeOut(tween(MemeDockMotion.Feedback)))
                    .using(SizeTransform(clip = false))
            }, label = "MemeDockPages") { destination ->
                CompositionLocalProvider(LocalStickerTransition provides StickerTransition(this@SharedTransitionLayout, this, page == destination)) {
                    if (destination.sticker != null) {
                        DetailRoute(destination.sticker, container, initialItem?.takeIf { it.id == destination.sticker }) { selected = null }
                    } else holder.SaveableStateProvider(destination.stateKey) {
                        Scaffold(contentWindowInsets = WindowInsets(0, 0, 0, 0),
                            bottomBar = { MemeDockBottomBar(destination.tab) { next ->
                                focus.clearFocus(); keyboard?.hide()
                                tab = next; collection = null; collectionName = null
                            } }) { padding ->
                            Box(Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding)) {
                                when (destination.tab) {
                                    HomeTab.Stickers -> LibraryRoute(container, open = open)
                                    HomeTab.Search -> SearchRoute(container, open)
                                    HomeTab.Collections -> if (destination.collection == null) CollectionsRoute(container) {
                                        collection = it.id; collectionName = it.name
                                    } else LibraryRoute(container, collectionId = destination.collection,
                                        title = destination.collectionName.orEmpty(), back = { collection = null; collectionName = null }, open = open)
                                    HomeTab.Settings -> SettingsRoute(container)
                                }
                            }
                        }
                    }
                }
            }
        }
        if (imports.visible) ImportSheet(imports, container.imports)
    }
}

private fun Context.activityWindow(): android.view.Window? = when (this) {
    is Activity -> window
    is ContextWrapper -> baseContext.activityWindow()
    else -> null
}
