package com.grtsinry43.memedock.ui.components

import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.SnackbarVisuals
import androidx.compose.runtime.Immutable

enum class MemeDockMessageType { Success, Warning, Error }

@Immutable
data class MemeDockMessage(
    override val message: String,
    val type: MemeDockMessageType,
    override val actionLabel: String? = null,
    override val withDismissAction: Boolean = true,
    override val duration: SnackbarDuration =
        if (actionLabel == null) SnackbarDuration.Short else SnackbarDuration.Indefinite,
) : SnackbarVisuals {
    init {
        require(message.isNotBlank()) { "Message text must not be blank" }
        require(actionLabel == null || actionLabel.isNotBlank()) { "Action label must not be blank" }
    }
}

/** Owned by the calling UI scope: cancellation removes a displayed or queued message. */
suspend fun SnackbarHostState.showMemeDockMessage(message: MemeDockMessage): SnackbarResult =
    showSnackbar(message)
