package com.grtsinry43.memedock.feature.detail

import com.grtsinry43.memedock.data.library.StickerDetails

data class DetailUiState(val loading: Boolean = true, val detail: StickerDetails? = null,
    val error: String? = null, val sharing: Boolean = false, val shareError: String? = null, val playing: Boolean = true, val shareLaunched: Boolean = false,
    val managing: Boolean = false, val managementError: String? = null)
