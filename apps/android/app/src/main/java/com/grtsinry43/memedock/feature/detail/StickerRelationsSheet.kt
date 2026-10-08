package com.grtsinry43.memedock.feature.detail

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
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
            MemeDockSectionHeader(stringResource(R.string.select_collections), Modifier.padding(top = MemeDockLayout.GapSmall), inset = 24.dp)
            val available = collections
            when {
                available == null -> if (loadError == null) MemeDockSheetLoading()
                available.isEmpty() -> MemeDockSheetHint(stringResource(R.string.no_collections_hint))
                else -> available.forEach { collection ->
                    val checked = collection.id == selectedCollection
                    MemeDockSheetChoice(collection.name, checked, { selectedCollection = if (checked) null else collection.id },
                        Modifier.testTag("relation-collection:${collection.id}"), icon = MemeDockIcons.Folder, enabled = !locked)
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
            MemeDockSectionHeader(stringResource(R.string.select_tags), Modifier.padding(top = MemeDockLayout.SectionGap), inset = 24.dp)
            val labels = tags
            if (labels == null) { if (loadError == null) MemeDockSheetLoading() }
            else FlowRow(Modifier.fillMaxWidth().padding(horizontal = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)) {
                labels.forEach { tag ->
                    MemeDockChoiceChip(tag.name, tag.id in selectedTags, multiple = true, enabled = !locked, onClick = {
                        selectedTags = if (tag.id in selectedTags) selectedTags - tag.id else selectedTags + tag.id
                    })
                }
                if (!addingTag) MemeDockAddChip(stringResource(R.string.new_tag),
                    { addingCollection = false; createError = null; addingTag = true },
                    Modifier.testTag("relations-new-tag"), enabled = !locked)
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
                MemeDockSheetError(failureText(code), Modifier.padding(top = MemeDockLayout.GapSmall),
                    retry = if (loadError != null) { { refresh++ } } else null)
            }
        }
        MemeDockButton(stringResource(R.string.done), { sheet.dismiss() },
            Modifier.padding(horizontal = 24.dp).padding(top = 20.dp).testTag("relations-done"), enabled = !creating, busy = busy)
    }
}
