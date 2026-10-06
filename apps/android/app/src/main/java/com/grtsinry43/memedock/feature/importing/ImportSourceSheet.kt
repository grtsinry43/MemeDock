package com.grtsinry43.memedock.feature.importing

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.components.MemeDockSheet
import com.grtsinry43.memedock.ui.components.MemeDockSheetAction

@Composable
fun ImportSourceSheet(visible: Boolean, dismiss: () -> Unit, photos: () -> Unit, files: () -> Unit) {
    MemeDockSheet(visible, dismiss, title = stringResource(R.string.import_photos),
        subtitle = stringResource(R.string.import_source_description)) {
        MemeDockSheetAction(stringResource(R.string.import_source_photos), MemeDockIcons.Image, photos,
            Modifier.testTag("import-source-photos"), supporting = stringResource(R.string.import_source_photos_hint))
        MemeDockSheetAction(stringResource(R.string.import_files), MemeDockIcons.Folder, files,
            Modifier.testTag("import-source-files"), supporting = stringResource(R.string.import_source_files_hint))
    }
}
