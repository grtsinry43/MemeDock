package com.grtsinry43.memedock.feature.library

import com.grtsinry43.memedock.data.library.LibraryCollection
import com.grtsinry43.memedock.data.library.LibraryItem

/** Home filters. Pages scoped to one collection ignore them. */
sealed interface LibraryFilter {
    data object Recent : LibraryFilter
    data object All : LibraryFilter
    data object Starred : LibraryFilter
    data class Collection(val id: String) : LibraryFilter
}

data class LibraryUiState(
    val search: String = "",
    val filter: LibraryFilter = LibraryFilter.Recent,
    val collections: List<LibraryCollection> = emptyList(),
    val items: List<LibraryItem> = emptyList(),
    val loading: Boolean = true,
    val loadingMore: Boolean = false,
    val hasMore: Boolean = false,
    val error: String? = null,
    val pageError: String? = null,
    val thumbnailEpoch: Long = 0,
)
