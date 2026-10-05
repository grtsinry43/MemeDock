package com.grtsinry43.memedock.feature.importing

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.MemeDockIcons

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ImportSourceSheet(dismiss: () -> Unit, photos: () -> Unit, files: () -> Unit) {
    ModalBottomSheet(onDismissRequest = dismiss) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.import_photos), style = MaterialTheme.typography.headlineSmall)
            Text(stringResource(R.string.import_source_description), color = MaterialTheme.colorScheme.onSurfaceVariant)
            Surface(onClick = photos, shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow) {
                ListItem(headlineContent = { Text(stringResource(R.string.import_source_photos)) },
                    supportingContent = { Text(stringResource(R.string.import_source_photos_hint)) },
                    leadingContent = { Icon(MemeDockIcons.Image, null, tint = MaterialTheme.colorScheme.primary) },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow))
            }
            Surface(onClick = files, shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow) {
                ListItem(headlineContent = { Text(stringResource(R.string.import_files)) },
                    supportingContent = { Text(stringResource(R.string.import_source_files_hint)) },
                    leadingContent = { Icon(MemeDockIcons.Folder, null, tint = MaterialTheme.colorScheme.primary) },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow))
            }
        }
    }
}
