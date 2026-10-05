package com.grtsinry43.memedock.feature.search

import androidx.compose.runtime.Composable
import com.grtsinry43.memedock.app.AppContainer
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.feature.library.LibraryRoute

@Composable
fun SearchRoute(container: AppContainer, open: (LibraryItem) -> Unit) {
    LibraryRoute(container, searchPage = true, open = open)
}
