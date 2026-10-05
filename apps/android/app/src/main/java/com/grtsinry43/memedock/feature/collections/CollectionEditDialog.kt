package com.grtsinry43.memedock.feature.collections

import androidx.compose.material3.*
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.failureText

@Composable
fun CollectionEditDialog(title: String, initial: String, busy: Boolean, error: String?, dismiss: () -> Unit, save: (String) -> Unit) {
    var name by rememberSaveable { mutableStateOf(initial) }
    AlertDialog(onDismissRequest = { if (!busy) dismiss() }, title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(name, { name = it }, enabled = !busy, singleLine = true,
                    label = { Text(stringResource(R.string.name)) }, modifier = Modifier.fillMaxWidth())
                error?.let { Text(failureText(it), color = MaterialTheme.colorScheme.error) }
            }
        }, dismissButton = { TextButton(onClick = dismiss, enabled = !busy) { Text(stringResource(R.string.cancel)) } },
        confirmButton = { TextButton(onClick = { save(name) }, enabled = name.isNotBlank() && !busy) {
            Text(stringResource(R.string.save))
        } })
}
