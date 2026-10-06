package com.grtsinry43.memedock.ui.components

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.AccessibilityManager
import androidx.compose.ui.platform.LocalAccessibilityManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R

/** App-wide message queue; mounted once at the root so messages survive page changes. */
val LocalMemeDockMessages = staticCompositionLocalOf<SnackbarHostState> { error("MemeDock message host is not mounted") }

/**
 * Bottom-anchored message bar. [bottomInset] lifts it above app chrome such as the tab bar; the
 * keyboard replaces that inset while it is open.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun MemeDockMessageHost(hostState: SnackbarHostState, modifier: Modifier = Modifier, bottomInset: Dp = 0.dp) {
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
    val lift = if (WindowInsets.isImeVisible) 0.dp else bottomInset
    CompositionLocalProvider(LocalAccessibilityManager provides adjustedManager) {
        SnackbarHost(hostState, modifier.windowInsetsPadding(WindowInsets.ime.union(WindowInsets.navigationBars))
            .padding(bottom = lift)) { data -> MessageBar(data) }
    }
}

@Composable
private fun MessageBar(data: SnackbarData) {
    val message = data.visuals as? MemeDockMessage
    val colors = MaterialTheme.colorScheme
    // The bar sits on inverseSurface in both themes, so status hues are picked for that background.
    val dark = colors.inverseSurface.luminance() < .5f
    val (icon, label, tint) = when (message?.type) {
        MemeDockMessageType.Success -> Triple(R.drawable.ic_message_success, R.string.message_success,
            if (dark) Color(0xFF5BD68A) else Color(0xFF1E8E4E))
        MemeDockMessageType.Warning -> Triple(R.drawable.ic_message_warning, R.string.message_warning,
            if (dark) Color(0xFFFFC94D) else Color(0xFFA86B00))
        MemeDockMessageType.Error -> Triple(R.drawable.ic_message_error, R.string.message_error,
            if (dark) Color(0xFFFF8A80) else Color(0xFFC5221F))
        null -> Triple(null, null, Color.Unspecified)
    }
    Surface(
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).widthIn(max = 560.dp).fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        color = colors.inverseSurface,
        contentColor = colors.inverseOnSurface,
        shadowElevation = 6.dp,
    ) {
        Row(
            Modifier.heightIn(min = 52.dp).padding(start = 16.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (icon != null && label != null) Icon(painterResource(icon), stringResource(label), Modifier.size(20.dp), tint = tint)
            Text(data.visuals.message, Modifier.weight(1f).padding(vertical = 8.dp),
                style = MaterialTheme.typography.bodyMedium)
            data.visuals.actionLabel?.let { action ->
                TextButton(onClick = data::performAction) { Text(action, color = colors.inversePrimary) }
            }
            if (data.visuals.withDismissAction) {
                IconButton(onClick = data::dismiss) {
                    Icon(painterResource(R.drawable.ic_message_close), stringResource(R.string.message_dismiss),
                        Modifier.size(20.dp), tint = colors.inverseOnSurface.copy(alpha = .7f))
                }
            }
        }
    }
}
