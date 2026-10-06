package com.grtsinry43.memedock.feature.detail

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.settings.ExportChoice

fun ExportChoice.titleResource() = when (this) {
    ExportChoice.Original -> R.string.export_original
    ExportChoice.CompatiblePng -> R.string.export_png
    ExportChoice.WhiteBackground -> R.string.export_white
    ExportChoice.SmallJpeg -> R.string.export_small
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ExportPresetSheet(selected: ExportChoice, dismiss: () -> Unit, select: (ExportChoice) -> Unit) {
    ModalBottomSheet(onDismissRequest = dismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).selectableGroup().padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.export_choose), style = MaterialTheme.typography.headlineSmall)
            ExportChoice.entries.forEach { choice ->
                val hint = when (choice) {
                    ExportChoice.Original -> R.string.export_original_hint
                    ExportChoice.CompatiblePng -> R.string.export_png_hint
                    ExportChoice.WhiteBackground -> R.string.export_white_hint
                    ExportChoice.SmallJpeg -> R.string.export_small_hint
                }
                Surface(shape = MaterialTheme.shapes.large,
                    color = if (choice == selected) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceContainerLow,
                    modifier = Modifier.fillMaxWidth()) {
                    Row(Modifier.fillMaxWidth().selectable(selected = choice == selected,
                        role = Role.RadioButton, onClick = { select(choice) })
                        .testTag("export-preset-${choice.name}").padding(16.dp)) {
                        RadioButton(selected = choice == selected, onClick = null)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(stringResource(choice.titleResource()), style = MaterialTheme.typography.titleMedium)
                            Text(stringResource(hint), style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
    }
}
