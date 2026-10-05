package com.grtsinry43.memedock.feature.detail

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.StickerDetails
import com.grtsinry43.memedock.ui.failureText

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StickerEditSheet(detail: StickerDetails, busy: Boolean, error: String?, dismiss: () -> Unit, save: (String?, String?) -> Unit) {
    // Keep the observed values with the draft, even if a notification reloads the detail.
    val original = remember(detail.id) { detail }
    var title by rememberSaveable(detail.id) { mutableStateOf(detail.title) }
    var note by rememberSaveable(detail.id) { mutableStateOf(detail.note) }
    ModalBottomSheet(onDismissRequest = { if (!busy) dismiss() },
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).imePadding().padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(stringResource(R.string.edit_sticker), style = MaterialTheme.typography.headlineSmall)
            OutlinedTextField(title, { title = it }, enabled = !busy, singleLine = true,
                label = { Text(stringResource(R.string.sticker_title)) }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(note, { note = it }, enabled = !busy, minLines = 3, maxLines = 6,
                label = { Text(stringResource(R.string.sticker_note)) }, modifier = Modifier.fillMaxWidth())
            error?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error) }
            Button(onClick = { save(title.takeIf { it != original.title }, note.takeIf { it != original.note }) },
                enabled = !busy && title.isNotBlank() && (title != original.title || note != original.note),
                modifier = Modifier.fillMaxWidth()) {
                if (busy) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else Text(stringResource(R.string.save))
            }
        }
    }
}
