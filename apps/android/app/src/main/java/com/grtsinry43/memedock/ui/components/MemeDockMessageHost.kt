package com.grtsinry43.memedock.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.AccessibilityManager
import androidx.compose.ui.platform.LocalAccessibilityManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.theme.LocalMemeDockSemanticColors

/** Place in Scaffold.snackbarHost; Scaffold handles system insets, imePadding adds only the IME remainder. */
@Composable
fun MemeDockMessageHost(hostState: SnackbarHostState, modifier: Modifier = Modifier) {
    val manager = LocalAccessibilityManager.current
    val adjustedManager = remember(manager, hostState) {
        manager?.let { delegate ->
            object : AccessibilityManager {
                override fun calculateRecommendedTimeoutMillis(
                    originalTimeoutMillis: Long,
                    containsIcons: Boolean,
                    containsText: Boolean,
                    containsControls: Boolean,
                ): Long = delegate.calculateRecommendedTimeoutMillis(
                    originalTimeoutMillis,
                    containsIcons,
                    containsText,
                    containsControls || hostState.currentSnackbarData?.visuals?.withDismissAction == true,
                )
            }
        }
    }
    CompositionLocalProvider(LocalAccessibilityManager provides adjustedManager) {
        SnackbarHost(hostState, modifier.imePadding()) { data ->
            val message = data.visuals as? MemeDockMessage
            val semantic = LocalMemeDockSemanticColors.current
            val colors = MaterialTheme.colorScheme
            val container = when (message?.type) {
                MemeDockMessageType.Success -> semantic.successContainer
                MemeDockMessageType.Warning -> semantic.warningContainer
                MemeDockMessageType.Error -> colors.errorContainer
                null -> colors.inverseSurface
            }
            val content = when (message?.type) {
                MemeDockMessageType.Success -> semantic.onSuccessContainer
                MemeDockMessageType.Warning -> semantic.onWarningContainer
                MemeDockMessageType.Error -> colors.onErrorContainer
                null -> colors.inverseOnSurface
            }
            val icon = when (message?.type) {
                MemeDockMessageType.Success -> R.drawable.ic_message_success
                MemeDockMessageType.Warning -> R.drawable.ic_message_warning
                MemeDockMessageType.Error -> R.drawable.ic_message_error
                null -> null
            }
            val typeLabel = when (message?.type) {
                MemeDockMessageType.Success -> R.string.message_success
                MemeDockMessageType.Warning -> R.string.message_warning
                MemeDockMessageType.Error -> R.string.message_error
                null -> null
            }
            Snackbar(
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                shape = MaterialTheme.shapes.medium,
                containerColor = container, contentColor = content,
                // Keep actions below the text: long Chinese messages and large fonts stay readable.
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalAlignment = Alignment.CenterVertically) {
                        if (icon != null && typeLabel != null) {
                            Icon(painterResource(icon), stringResource(typeLabel),
                                Modifier.size(20.dp), tint = content)
                        }
                        Text(data.visuals.message, style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.weight(1f))
                    }
                    if (data.visuals.actionLabel != null || data.visuals.withDismissAction) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End,
                            verticalAlignment = Alignment.CenterVertically) {
                            data.visuals.actionLabel?.let { label ->
                                TextButton(onClick = data::performAction) { Text(label, color = content) }
                            }
                            if (data.visuals.withDismissAction) {
                                IconButton(onClick = data::dismiss) {
                                    Icon(painterResource(R.drawable.ic_message_close),
                                        stringResource(R.string.message_dismiss), tint = content)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
