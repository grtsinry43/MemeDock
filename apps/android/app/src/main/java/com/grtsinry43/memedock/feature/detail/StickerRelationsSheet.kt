package com.grtsinry43.memedock.feature.detail

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureCode
import com.grtsinry43.memedock.ui.failureText
import com.grtsinry43.memedock.ui.theme.MemeDockLayout
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/**
 * Picks collections and tags for [detail]; shown while it is non-null. Closing the sheet in any way saves the
 * selection: unchanged selections just [close], otherwise [save] runs and the caller clears [detail] on success.
 * A failed save keeps the sheet open with [error].
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StickerRelationsSheet(detail: StickerDetails?, repository: ManagementRepository, busy: Boolean, error: String?,
    close: () -> Unit, save: (LibraryCollection?, List<LibraryTag>) -> Unit) {
    val target = rememberRetained(detail) ?: return
    val visible = detail != null
    var collections by remember(target.id) { mutableStateOf<List<LibraryCollection>?>(null) }
    var tags by remember(target.id) { mutableStateOf<List<LibraryTag>?>(null) }
    var selectedCollection by remember(target) { mutableStateOf(target.collection?.id) }
    var selectedTags by remember(target) { mutableStateOf(target.tags.map { it.id }.toSet()) }
    var loadError by remember(target.id) { mutableStateOf<String?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    var addingCollection by remember(target.id) { mutableStateOf(false) }
    var addingTag by remember(target.id) { mutableStateOf(false) }
    var creating by remember { mutableStateOf(false) }
    var createError by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(target.id, visible, refresh) {
        if (!visible) return@LaunchedEffect
        loadError = null
        try { collections = repository.collections(false); tags = repository.tags() }
        catch (cancel: CancellationException) { throw cancel }
        catch (failure: Exception) { loadError = failureCode(failure) }
    }
    val loaded = collections != null && tags != null
    val changed = loaded && (selectedCollection != target.collection?.id ||
        selectedTags != target.tags.map { it.id }.toSet())
    val locked = busy || creating
    MemeDockSheet(
        visible = visible,
        onDismissRequest = close,
        title = stringResource(R.string.organize),
        subtitle = target.title,
        dismissible = !locked,
        confirmDismiss = {
            when {
                locked -> false
                !changed -> true
                else -> {
                    save(collections.orEmpty().firstOrNull { it.id == selectedCollection }, tags.orEmpty().filter { it.id in selectedTags })
                    false
                }
            }
        },
    ) {
        val sheet = this
        Column(Modifier.weight(1f, fill = false).imePadding().verticalScroll(rememberScrollState())) {
            SectionLabel(stringResource(R.string.select_collections))
            val available = collections
            when {
                available == null -> if (loadError == null) LoadingRows()
                available.isEmpty() -> EmptyHint(stringResource(R.string.no_collections_hint))
                else -> available.forEach { collection ->
                    val checked = collection.id == selectedCollection
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = MemeDockLayout.RowHeight)
                            .toggleable(checked, enabled = !locked, role = Role.RadioButton) {
                                selectedCollection = if (checked) null else collection.id
                            }
                            .padding(horizontal = 24.dp).testTag("relation-collection:${collection.id}"),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        Icon(MemeDockIcons.Folder, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(collection.name, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge,
                            maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (checked) Icon(MemeDockIcons.Check, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.primary)
                    }
                }
            }
            if (available != null) MemeDockInlineCreate(
                label = stringResource(R.string.new_collection),
                placeholder = stringResource(R.string.name),
                editing = addingCollection,
                onStart = { if (!locked) { addingTag = false; createError = null; addingCollection = true } },
                onCreate = { name ->
                    if (!locked) {
                        creating = true
                        createError = null
                        scope.launch {
                            try {
                                val collection = repository.createCollection(name)
                                collections = (collections.orEmpty() + collection).distinctBy { it.id }
                                selectedCollection = collection.id
                                addingCollection = false
                            } catch (cancel: CancellationException) { throw cancel }
                            catch (failure: Exception) { createError = failureCode(failure) }
                            finally { creating = false }
                        }
                    }
                },
                onCancel = { addingCollection = false; createError = null },
                modifier = Modifier.testTag("relations-new-collection"),
                busy = creating,
                error = createError?.let { failureText(it) },
            )
            SectionLabel(stringResource(R.string.select_tags), Modifier.padding(top = 16.dp))
            val labels = tags
            if (labels == null) { if (loadError == null) LoadingRows() }
            else FlowRow(Modifier.fillMaxWidth().padding(horizontal = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)) {
                labels.forEach { tag ->
                    TagChip(tag.name, tag.id in selectedTags, !locked) {
                        selectedTags = if (tag.id in selectedTags) selectedTags - tag.id else selectedTags + tag.id
                    }
                }
                if (!addingTag) AddTagChip(enabled = !locked) { addingCollection = false; createError = null; addingTag = true }
            }
            if (labels != null && addingTag) MemeDockInlineEdit(
                initial = "",
                placeholder = stringResource(R.string.new_tag),
                onCommit = { name ->
                    creating = true
                    createError = null
                    scope.launch {
                        try {
                            val tag = repository.createTag(name)
                            tags = repository.tags()
                            selectedTags = selectedTags + tag.id
                            addingTag = false
                        } catch (cancel: CancellationException) { throw cancel }
                        catch (failure: Exception) { createError = failureCode(failure) }
                        finally { creating = false }
                    }
                },
                onCancel = { addingTag = false; createError = null },
                modifier = Modifier.padding(horizontal = 24.dp).padding(top = 12.dp),
                busy = creating,
                error = createError?.let { failureText(it) },
            )
            (error ?: loadError)?.let { code ->
                Row(Modifier.fillMaxWidth().padding(start = 24.dp, end = 12.dp, top = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(failureText(code), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error)
                    if (loadError != null) TextButton(onClick = { refresh++ }) { Text(stringResource(R.string.retry)) }
                }
            }
        }
        Button(
            onClick = { if (!locked) sheet.dismiss() },
            enabled = !locked,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(top = 20.dp).height(52.dp)
                .testTag("relations-done"),
            shape = MaterialTheme.shapes.medium,
        ) {
            if (busy) CircularProgressIndicator(Modifier.size(20.dp), color = MaterialTheme.colorScheme.onPrimary, strokeWidth = 2.dp)
            else Text(stringResource(R.string.done))
        }
    }
}

@Composable
private fun SectionLabel(text: String, modifier: Modifier = Modifier) {
    Text(text, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.padding(horizontal = 24.dp, vertical = 8.dp).semantics { heading() })
}

@Composable
private fun EmptyHint(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
}

@Composable
private fun LoadingRows() {
    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        MemeDockSkeleton(Modifier.fillMaxWidth(.6f).height(20.dp), MaterialTheme.shapes.extraSmall)
        MemeDockSkeleton(Modifier.fillMaxWidth(.4f).height(20.dp), MaterialTheme.shapes.extraSmall)
    }
}

@Composable
private fun TagChip(name: String, selected: Boolean, enabled: Boolean, toggle: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.heightIn(min = 36.dp).clip(MaterialTheme.shapes.small)
            .background(if (selected) colors.primary.copy(alpha = .12f) else colors.surfaceContainer)
            .toggleable(selected, enabled = enabled, role = Role.Checkbox) { toggle() }
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (selected) Icon(MemeDockIcons.Check, null, Modifier.size(16.dp), tint = colors.primary)
        Text(name, style = MaterialTheme.typography.bodyMedium, color = if (selected) colors.primary else colors.onSurface,
            maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun AddTagChip(enabled: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.heightIn(min = 36.dp).clip(MaterialTheme.shapes.small).background(colors.primary.copy(alpha = .10f))
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(start = 8.dp, end = 12.dp).testTag("relations-new-tag"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(MemeDockIcons.Add, null, Modifier.size(16.dp), tint = colors.primary)
        Text(stringResource(R.string.new_tag), style = MaterialTheme.typography.bodyMedium, color = colors.primary)
    }
}
