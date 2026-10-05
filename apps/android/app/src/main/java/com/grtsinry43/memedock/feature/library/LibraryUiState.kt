package com.grtsinry43.memedock.feature.library

import com.grtsinry43.memedock.data.library.LibraryItem

data class LibraryUiState(
    val search: String = "",
    val items: List<LibraryItem> = emptyList(),
    val loading: Boolean = true,
    val loadingMore: Boolean = false,
    val hasMore: Boolean = false,
    val error: String? = null,
    val pageError: String? = null,
    val thumbnailEpoch: Long = 0,
)
