package com.grtsinry43.memedock.feature.detail

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.ui.components.MemeDockIcons
import com.grtsinry43.memedock.ui.components.MemeDockSheet
import com.grtsinry43.memedock.ui.theme.MemeDockLayout

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
                Row(
                    Modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight)
                        .selectable(checked, role = Role.RadioButton, onClick = { if (checked) dismiss() else select(choice) })
                        .testTag("export-preset-${choice.name}").padding(horizontal = 24.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(stringResource(choice.titleResource()), style = MaterialTheme.typography.bodyLarge,
                            color = if (checked) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
                        Text(stringResource(choice.hintResource()), style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    if (checked) Icon(MemeDockIcons.Check, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.primary)
                    else Spacer(Modifier.size(22.dp))
                }
            }
        }
    }
}
