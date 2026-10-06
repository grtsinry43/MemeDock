package com.grtsinry43.memedock.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.*
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import com.grtsinry43.memedock.ui.theme.MemeDockMotion
import dev.chrisbanes.haze.HazeState

/** Secondary-page bar. Pass [scrolled] once content sits under the bar to reveal the hairline. */
@Composable
fun MemeDockTopBar(
    title: String,
    glass: HazeState,
    modifier: Modifier = Modifier,
    back: (() -> Unit)? = null,
    scrolled: Boolean = false,
    actions: @Composable RowScope.() -> Unit = {},
) {
    val divider by animateFloatAsState(if (scrolled) 1f else 0f, tween(MemeDockMotion.Feedback), label = "top-bar-divider")
    Column(modifier.fillMaxWidth().glass(glass, glassStyle())) {
        Row(
            Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal))
                .fillMaxWidth().height(MemeDockLayout.TopBarHeight)
                .padding(start = if (back == null) MemeDockLayout.PagePadding else 4.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (back != null) {
                IconButton(onClick = back) { Icon(MemeDockIcons.Back, stringResource(R.string.back)) }
                Spacer(Modifier.width(4.dp))
            }
            Text(
                title,
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                maxLines = 1, overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).semantics { heading() },
            )
            actions()
        }
        HorizontalDivider(Modifier.alpha(divider), MemeDockLayout.Hairline, MaterialTheme.colorScheme.outlineVariant)
    }
}
