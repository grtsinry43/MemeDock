package com.grtsinry43.memedock.ui

import android.content.Context
import android.content.ContextWrapper
import android.content.res.AssetManager
import android.content.res.Resources
import androidx.annotation.StringRes
import androidx.activity.compose.BackHandler
import androidx.compose.animation.*
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.navigation3.rememberViewModelStoreNavEntryDecorator
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.runtime.rememberSaveableStateHolderNavEntryDecorator
import androidx.navigation3.ui.NavDisplay
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.settings.ThemeMode
import com.grtsinry43.memedock.feature.backup.BackupPhase
import com.grtsinry43.memedock.feature.backup.BackupRoute
import com.grtsinry43.memedock.feature.detail.DetailRoute
import com.grtsinry43.memedock.feature.importing.ImportSheet
import com.grtsinry43.memedock.feature.library.GroupLibraryRoute
import com.grtsinry43.memedock.feature.library.HomeLibraryRoute
import com.grtsinry43.memedock.feature.library.StickerGroup
import com.grtsinry43.memedock.feature.organize.OrganizeRoute
import com.grtsinry43.memedock.feature.settings.SettingsRoute
import com.grtsinry43.memedock.feature.trash.TrashRoute
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.navigation.BackStackSaver
import com.grtsinry43.memedock.ui.navigation.Destination
import com.grtsinry43.memedock.ui.navigation.forwardPage
import com.grtsinry43.memedock.ui.navigation.backwardPage
import com.grtsinry43.memedock.ui.navigation.tabTransition
import com.grtsinry43.memedock.ui.navigation.stickerPageTransitions
import com.grtsinry43.memedock.ui.theme.*
import dev.chrisbanes.haze.rememberHazeState
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.retryWhen
import kotlinx.coroutines.launch
import java.io.IOException

import android.content.res.Configuration
import android.os.LocaleList
import androidx.compose.ui.platform.LocalConfiguration
import com.grtsinry43.memedock.data.settings.LanguageMode
import java.util.Locale

@Composable
fun MemeDockApp(container: AppContainer) {
    val epoch by container.libraryEpoch.collectAsStateWithLifecycle()
    // Outside key(epoch): a message about a restore must outlive the library it replaced.
    val messages = remember { SnackbarHostState() }
    val localizedContext = rememberLocalizedContext(container)
    CompositionLocalProvider(
        LocalMemeDockMessages provides messages,
        LocalContext provides localizedContext,
        LocalConfiguration provides localizedContext.resources.configuration,
        LocalResources provides localizedContext.resources,
    ) {
        key(epoch) {
            val owner = remember { object : androidx.lifecycle.ViewModelStoreOwner {
                override val viewModelStore = androidx.lifecycle.ViewModelStore()
            } }
            DisposableEffect(owner) { onDispose { owner.viewModelStore.clear() } }
            LaunchedEffect(epoch) { if (epoch > 0) container.imageLoader.memoryCache?.clear() }
            CompositionLocalProvider(androidx.lifecycle.viewmodel.compose.LocalViewModelStoreOwner provides owner) {
                MemeDockContent(container, messages)
            }
        }
    }
}

@Composable
private fun MemeDockContent(container: AppContainer, messages: SnackbarHostState) {
    val direction = if (LocalLayoutDirection.current == LayoutDirection.Rtl) -1 else 1
    val restoring = remember(container) { container.backups.state.value.phase == BackupPhase.Restoring }
    val backStack = rememberSaveable(saver = BackStackSaver) {
        if (restoring) mutableStateListOf(Destination.Home, Destination.Backup) else mutableStateListOf(Destination.Home)
    }
    var tab by rememberSaveable { mutableStateOf(if (restoring) HomeTab.Mine else HomeTab.Stickers) }
    // The tapped tile seeds the detail transition; it is display state, not navigation state.
    var initialItem by remember { mutableStateOf<LibraryItem?>(null) }
    val keyboard = LocalSoftwareKeyboardController.current
    val focus = LocalFocusManager.current
    val imports by container.imports.state.collectAsStateWithLifecycle()
    val dark = rememberDarkTheme(container)
    fun push(destination: Destination) { focus.clearFocus(); keyboard?.hide(); backStack.add(destination) }
    fun pop() { if (backStack.size > 1) backStack.removeAt(backStack.lastIndex) }
    val open: (LibraryItem) -> Unit = { item -> initialItem = item; push(Destination.Sticker(item.id)) }
    val scope = rememberCoroutineScope()
    val resources = LocalContext.current.resources
    /** Leaves a page whose subject just moved to the trash, offering to follow it there. */
    fun deleted(@StringRes text: Int) {
        pop()
        val message = MemeDockMessage(resources.getString(text), MemeDockMessageType.Success,
            actionLabel = resources.getString(R.string.view), duration = SnackbarDuration.Long)
        scope.launch { if (messages.showMemeDockMessage(message) == SnackbarResult.ActionPerformed) push(Destination.Trash) }
    }
    val groupDeleted: (StickerGroup) -> Unit = { group ->
        deleted(if (group is StickerGroup.Tag) R.string.tag_deleted else R.string.collection_deleted)
    }
    LaunchedEffect(container.imports) {
        container.imports.reports.collect { report ->
            val problems = report.unresolved > 0 && !report.stopped
            val text = when {
                report.stopped -> resources.getString(R.string.import_stopped, report.created)
                problems -> resources.getString(R.string.import_done_partial, report.created + report.reused, report.unresolved)
                report.created == 0 -> resources.getString(R.string.import_done_existing)
                report.reused > 0 -> resources.getString(R.string.import_done_reused, report.created, report.reused)
                else -> resources.getString(R.string.import_done, report.created)
            }
            val message = MemeDockMessage(text, if (problems) MemeDockMessageType.Warning else MemeDockMessageType.Success,
                actionLabel = if (problems) resources.getString(R.string.view) else null,
                duration = if (problems) SnackbarDuration.Long else SnackbarDuration.Short)
            // Reports queue behind each other; a later batch never replaces the summary of an earlier one.
            if (messages.showMemeDockMessage(message) == SnackbarResult.ActionPerformed) container.imports.show()
        }
    }
    LaunchedEffect(imports.selectionError) {
        val code = imports.selectionError ?: return@LaunchedEffect
        container.imports.clearSelectionError()
        val message = MemeDockMessage(resources.getString(failureTextRes(code)), MemeDockMessageType.Error)
        scope.launch { messages.showMemeDockMessage(message) }
    }
    MemeDockTheme(darkTheme = dark) {
        Box(Modifier.fillMaxSize()) {
            SharedTransitionLayout {
                val shared = this
                NavDisplay(
                    backStack = backStack,
                    modifier = Modifier.fillMaxSize(),
                    onBack = ::pop,
                    entryDecorators = listOf(rememberSaveableStateHolderNavEntryDecorator(), rememberViewModelStoreNavEntryDecorator()),
                    sharedTransitionScope = shared,
                    transitionSpec = { forwardPage(direction) },
                    popTransitionSpec = { backwardPage(direction) },
                    predictivePopTransitionSpec = { _ -> backwardPage(direction) },
                    entryProvider = { destination ->
                        NavEntry(destination, metadata = if (destination is Destination.Sticker) stickerPageTransitions else emptyMap()) {
                            ProvideStickerTransition(shared) {
                                when (destination) {
                                    Destination.Home -> HomeScreen(container, tab, { next ->
                                        focus.clearFocus(); keyboard?.hide(); tab = next
                                    }, open, ::push)
                                    is Destination.Sticker -> DetailRoute(destination.id, container,
                                        initialItem?.takeIf { it.id == destination.id }, ::pop) { deleted(R.string.sticker_deleted) }
                                    is Destination.Collection -> GroupLibraryRoute(container, StickerGroup.Collection(destination.id),
                                        destination.name, ::pop, open, { push(Destination.Trash) }, groupDeleted)
                                    is Destination.Tag -> GroupLibraryRoute(container, StickerGroup.Tag(destination.id),
                                        destination.name, ::pop, open, { push(Destination.Trash) }, groupDeleted)
                                    Destination.Collections -> OrganizeRoute(container, PaddingValues(), { push(Destination.Collection(it.id, it.name)) },
                                        { push(Destination.Tag(it.id, it.name)) }, { push(Destination.Trash) }, allCollections = true, back = ::pop)
                                    Destination.Trash -> TrashRoute(container, ::pop, open)
                                    Destination.Backup -> BackupRoute(container.backups, ::pop)
                                }
                            }
                        }
                    },
                )
            }
            MemeDockMessageHost(messages, Modifier.align(Alignment.BottomCenter),
                bottomInset = if (backStack.last() == Destination.Home) MemeDockLayout.BottomBarHeight else 0.dp)
            ImportSheet(imports, container.imports, container.library, container.imageLoader)
        }
    }
}

@Composable
private fun HomeScreen(container: AppContainer, tab: HomeTab, select: (HomeTab) -> Unit,
    open: (LibraryItem) -> Unit, push: (Destination) -> Unit) {
    val tabs = rememberSaveableStateHolder()
    val layoutDirection = if (LocalLayoutDirection.current == LayoutDirection.Rtl) -1 else 1
    BackHandler(enabled = tab != HomeTab.Stickers) { select(HomeTab.Stickers) }
    val glass = rememberHazeState()
    // Tab content runs under the frosted bar; it pads its own scrollable content by this much.
    val bar = PaddingValues(bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding() +
        MemeDockLayout.BottomBarHeight + MemeDockLayout.Hairline)
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().glassSource(glass)) {
            AnimatedContent(targetState = tab, modifier = Modifier.fillMaxSize(),
                transitionSpec = { tabTransition((if (targetState.ordinal > initialState.ordinal) 1 else -1) * layoutDirection) },
                contentKey = { it }, label = "home-tabs") { shownTab ->
                tabs.SaveableStateProvider(shownTab.name) {
                    when (shownTab) {
                        HomeTab.Stickers -> HomeLibraryRoute(container, bar, open) { push(Destination.Trash) }
                        HomeTab.Organize -> OrganizeRoute(container, bar, { push(Destination.Collection(it.id, it.name)) },
                            { push(Destination.Tag(it.id, it.name)) }, { push(Destination.Trash) }, viewAll = { push(Destination.Collections) })
                        HomeTab.Mine -> SettingsRoute(container, bar, { push(Destination.Trash) }, { push(Destination.Backup) })
                    }
                }
            }
        }
        MemeDockBottomBar(tab, select, glass, Modifier.align(Alignment.BottomCenter))
    }
}

@Composable
private fun rememberDarkTheme(container: AppContainer): Boolean {
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
    return dark
}

private fun Context.activityWindow(): android.view.Window? = findActivity()?.window

@Composable
private fun rememberLocalizedContext(container: AppContainer): Context {
    val baseContext = LocalContext.current
    val baseConfiguration = LocalConfiguration.current
    val languageMode by remember(container) {
        container.language.mode.retryWhen { error, _ ->
            if (error is IOException) { delay(1_000); true } else false
        }.catch { emit(LanguageMode.System) }
    }.collectAsStateWithLifecycle(LanguageMode.System)

    return remember(baseContext, baseConfiguration, languageMode) {
        if (languageMode == LanguageMode.System) baseContext else {
            val config = Configuration(baseConfiguration)
            val locale = if (languageMode == LanguageMode.Chinese) Locale.SIMPLIFIED_CHINESE else Locale.ENGLISH
            config.setLocales(LocaleList(locale))
            val localized = baseContext.createConfigurationContext(config)
            // Keep the Activity in the context chain for result launchers,
            // sharing and window access; replace only locale-aware resources.
            object : ContextWrapper(baseContext) {
                override fun getResources(): Resources = localized.resources
                override fun getAssets(): AssetManager = localized.assets
            }
        }
    }
}
