package com.grtsinry43.memedock.feature.detail

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.ui.components.MemeDockSheet
import com.grtsinry43.memedock.ui.components.MemeDockSheetChoice

fun ExportChoice.titleResource() = when (this) {
    ExportChoice.Original -> R.string.export_original
    ExportChoice.CompatiblePng -> R.string.export_png
    ExportChoice.WhiteBackground -> R.string.export_white
    ExportChoice.SmallJpeg -> R.string.export_small
}

private fun ExportChoice.hintResource() = when (this) {
    ExportChoice.Original -> R.string.export_original_hint
    ExportChoice.CompatiblePng -> R.string.export_png_hint
    ExportChoice.WhiteBackground -> R.string.export_white_hint
    ExportChoice.SmallJpeg -> R.string.export_small_hint
}

/** Picking a format applies it right away; the caller closes the sheet once the choice is stored. */
@Composable
fun ExportPresetSheet(visible: Boolean, selected: ExportChoice, dismiss: () -> Unit, select: (ExportChoice) -> Unit) {
    MemeDockSheet(visible, dismiss, title = stringResource(R.string.export_choose)) {
        Column(Modifier.fillMaxWidth().selectableGroup()) {
            ExportChoice.entries.forEach { choice ->
                val checked = choice == selected
                MemeDockSheetChoice(stringResource(choice.titleResource()), checked, { if (checked) dismiss() else select(choice) },
                    Modifier.testTag("export-preset-${choice.name}"), supporting = stringResource(choice.hintResource()))
            }
        }
    }
}
