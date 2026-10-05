package com.grtsinry43.memedock.feature.detail

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.failureText
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StickerRelationsSheet(detail: StickerDetails, repository: ManagementRepository, busy: Boolean, error: String?,
    dismiss: () -> Unit, save: (List<LibraryCollection>, List<LibraryTag>) -> Unit) {
    var collections by remember { mutableStateOf<List<LibraryCollection>?>(null) }
    var tags by remember { mutableStateOf<List<LibraryTag>?>(null) }
    val selectionSaver = listSaver<List<String>, String>(save = { it }, restore = { it.toList() })
    var selectedCollections by rememberSaveable(detail.id, stateSaver = selectionSaver) { mutableStateOf(detail.collections.map { it.id }) }
    var selectedTags by rememberSaveable(detail.id, stateSaver = selectionSaver) { mutableStateOf(detail.tags.map { it.id }) }
    var loadError by remember { mutableStateOf<String?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    var creating by remember { mutableStateOf(false) }
    var tagName by rememberSaveable { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    LaunchedEffect(repository, refresh) {
        loadError = null
        try { collections = repository.collections(false); tags = repository.tags() }
        catch (cancel: CancellationException) { throw cancel }
        catch (failure: Exception) { loadError = (failure as? LibraryFailure)?.reason ?: "INTERNAL" }
    }
    ModalBottomSheet(onDismissRequest = { if (!busy && !creating) dismiss() },
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().heightIn(max = 650.dp).imePadding().padding(horizontal = 24.dp).padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.organize), style = MaterialTheme.typography.headlineSmall)
            LazyColumn(Modifier.weight(1f, fill = false)) {
                item { Text(stringResource(R.string.select_collections), style = MaterialTheme.typography.titleMedium) }
                if (collections?.isEmpty() == true) item {
                    Text(stringResource(R.string.choices_empty), Modifier.padding(vertical = 12.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                items(collections.orEmpty(), key = { "collection:" + it.id }) { value ->
                    RelationChoice(value.name, value.id in selectedCollections, !busy && !creating) {
                        selectedCollections = if (value.id in selectedCollections) selectedCollections - value.id else selectedCollections + value.id
                    }
                }
                item { Text(stringResource(R.string.select_tags), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 12.dp)) }
                if (tags?.isEmpty() == true) item {
                    Text(stringResource(R.string.choices_empty), Modifier.padding(vertical = 12.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                items(tags.orEmpty(), key = { "tag:" + it.id }) { value ->
                    RelationChoice(value.name, value.id in selectedTags, !busy && !creating) {
                        selectedTags = if (value.id in selectedTags) selectedTags - value.id else selectedTags + value.id
                    }
                }
                item {
                    if (collections == null || tags == null) {
                        if (loadError == null) CircularProgressIndicator(Modifier.padding(16.dp))
                    }
                    OutlinedTextField(tagName, { tagName = it }, enabled = !busy && !creating, singleLine = true,
                        label = { Text(stringResource(R.string.new_tag)) }, modifier = Modifier.fillMaxWidth().padding(top = 12.dp))
                    TextButton(enabled = tagName.isNotBlank() && !busy && !creating, onClick = {
                        creating = true
                        scope.launch {
                            try {
                                val tag = repository.createTag(tagName)
                                tags = repository.tags()
                                selectedTags = (selectedTags + tag.id).distinct()
                                tagName = ""; loadError = null
                            } catch (cancel: CancellationException) { throw cancel }
                            catch (failure: Exception) { loadError = (failure as? LibraryFailure)?.reason ?: "INTERNAL" }
                            finally { creating = false }
                        }
                    }) { Text(stringResource(R.string.new_tag)) }
                }
            }
            (error ?: loadError)?.let {
                Text(failureText(it), color = MaterialTheme.colorScheme.error)
                if (loadError != null) TextButton(onClick = { refresh++ }, enabled = !creating) { Text(stringResource(R.string.retry)) }
            }
            Button(onClick = {
                save(collections.orEmpty().filter { it.id in selectedCollections }, tags.orEmpty().filter { it.id in selectedTags })
            }, enabled = !busy && !creating && collections != null && tags != null && loadError == null,
                modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.save)) }
        }
    }
}

@Composable
private fun RelationChoice(name: String, selected: Boolean, enabled: Boolean, toggle: () -> Unit) {
    Row(Modifier.fillMaxWidth().toggleable(selected, enabled = enabled, role = Role.Checkbox, onValueChange = { toggle() })
        .padding(vertical = 4.dp), verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
        Checkbox(selected, onCheckedChange = null, enabled = enabled)
        Text(name, Modifier.padding(start = 8.dp).weight(1f))
    }
}
