package com.grtsinry43.memedock.feature.library

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryItem
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.feature.detail.titleResource
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.components.MemeDockSheet
import com.grtsinry43.memedock.ui.components.MemeDockSheetAction
import com.grtsinry43.memedock.ui.components.rememberRetained

/**
 * Long-press actions. Animated stickers keep their animation: a static preset would flatten them,
 * so plain share and copy send the original and a separate action offers the first frame.
 */
@Composable
fun StickerQuickSheet(
    item: LibraryItem?,
    choice: ExportChoice,
    onDismissRequest: () -> Unit,
    share: (LibraryItem, ExportChoice, Boolean) -> Unit,
    copy: (LibraryItem, ExportChoice) -> Unit,
    star: (LibraryItem) -> Unit,
    organize: (LibraryItem) -> Unit,
    delete: (LibraryItem) -> Unit,
    select: (LibraryItem) -> Unit = {},
) {
    val shown = rememberRetained(item) ?: return
    val flattens = shown.animated && choice != ExportChoice.Original
    val sent = if (flattens) ExportChoice.Original else choice
    val format = stringResource(choice.titleResource())
    MemeDockSheet(item != null, onDismissRequest, title = shown.title) {
        fun act(action: () -> Unit) { action(); dismiss() }
        MemeDockSheetAction(stringResource(R.string.select_items), MemeDockIcons.Select, { act { select(shown) } })
        MemeDockSheetAction(stringResource(R.string.share_sticker), MemeDockIcons.Share,
            { act { share(shown, sent, false) } },
            supporting = if (flattens) stringResource(R.string.quick_animated_original) else format)
        if (flattens) MemeDockSheetAction(stringResource(R.string.share_first_frame), MemeDockIcons.Image,
            { act { share(shown, choice, true) } }, supporting = format)
        MemeDockSheetAction(stringResource(R.string.copy_image), MemeDockIcons.Copy, { act { copy(shown, sent) } })
        MemeDockSheetAction(stringResource(if (shown.starred) R.string.unfavorite else R.string.favorite),
            if (shown.starred) MemeDockIcons.StarFilled else MemeDockIcons.Star,
            { act { star(shown) } })
        MemeDockSheetAction(stringResource(R.string.organize), MemeDockIcons.Collections, { act { organize(shown) } },
            supporting = stringResource(R.string.organize_hint))
        MemeDockSheetAction(stringResource(R.string.delete), MemeDockIcons.Delete, { act { delete(shown) } }, destructive = true)
    }
}
