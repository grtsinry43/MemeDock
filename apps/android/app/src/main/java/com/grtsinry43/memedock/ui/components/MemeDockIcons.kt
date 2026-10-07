package com.grtsinry43.memedock.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.vectorResource
import com.grtsinry43.memedock.R
import com.composables.icons.materialsymbols.rounded.R as Symbols

object MemeDockIcons {
    val Select: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_select_check_box_rounded)
    val Copy: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_content_copy_rounded)
    val Download: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_download_rounded)
    // Back, Check and Star are local copies: material-symbols 2.2.1 converts paths that open with a relative
    // move with their first point outside the viewport.
    val Star: ImageVector @Composable get() = ImageVector.vectorResource(R.drawable.ic_star)
    val StarFilled: ImageVector @Composable get() = ImageVector.vectorResource(R.drawable.ic_star_filled)
    val More: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_more_vert_rounded)
    val Label: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_label_rounded)
    val Edit: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_edit_rounded)
    val Back: ImageVector @Composable get() = ImageVector.vectorResource(R.drawable.ic_back)
    val Add: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_add_rounded)
    val Close: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_close_rounded)
    val Search: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_search_rounded)
    val Image: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_image_rounded)
    val Folder: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_folder_rounded)
    val Share: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_ios_share_rounded)
    val Play: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_play_arrow_rounded)
    val Pause: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_pause_rounded)
    val Check: ImageVector @Composable get() = ImageVector.vectorResource(R.drawable.ic_check)
    val Alert: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_warning_rounded)
    val Mood: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_mood_rounded)
    val Person: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_person_rounded)
    val Collections: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_collections_bookmark_rounded)
    val ChevronRight: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_chevron_right_rounded)
    val DragHandle: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_drag_handle_rounded)
    val ExpandMore: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_expand_more_rounded)
    val Delete: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_delete_rounded)
    val Archive: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_archive_rounded)
    val Unarchive: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_unarchive_rounded)
    val BackupRestore: ImageVector @Composable get() = ImageVector.vectorResource(Symbols.drawable.materialsymbols_ic_settings_backup_restore_rounded)
}
