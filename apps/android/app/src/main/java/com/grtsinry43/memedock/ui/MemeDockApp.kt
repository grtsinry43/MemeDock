package com.grtsinry43.memedock.ui

import androidx.compose.runtime.Composable
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.feature.library.LibraryRoute
import com.grtsinry43.memedock.ui.theme.MemeDockTheme

@Composable
fun MemeDockApp(container: AppContainer) {
    MemeDockTheme {
        LibraryRoute(container)
    }
}
