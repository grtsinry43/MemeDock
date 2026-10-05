package com.grtsinry43.memedock.feature.collections

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.*
import com.grtsinry43.memedock.ui.failureText
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CollectionOrderSheet(id: String, library: LibraryRepository, management: ManagementRepository, dismiss: () -> Unit) {
    var items by remember(id) { mutableStateOf<List<LibraryItem>>(emptyList()) }
    var collection by remember(id) { mutableStateOf<LibraryCollection?>(null) }
    var loading by remember { mutableStateOf(true) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    var moving by remember { mutableStateOf<LibraryItem?>(null) }
    var cursor by remember { mutableStateOf<PageCursor?>(null) }
    var first by remember { mutableStateOf<LibraryItem?>(null) }
    val scope = rememberCoroutineScope()
    DisposableEffect(id) { onDispose { cursor?.close() } }
    LaunchedEffect(id, refresh) {
        loading = true; error = null
        cursor?.close(); cursor = null
        try {
            collection = management.collections(false).firstOrNull { it.id == id } ?: throw LibraryFailure("ENTITY_DELETED")
            val page = library.page("", collectionId = id)
            cursor = page.next; items = page.items; first = page.items.firstOrNull()
        } catch (cancel: CancellationException) { throw cancel }
        catch (failure: Exception) { error = (failure as? LibraryFailure)?.reason ?: "INTERNAL" }
        finally { loading = false }
    }
    fun move(value: LibraryItem, before: LibraryItem?) {
        val target = collection ?: return
        if (busy) return
        busy = true; error = null
        scope.launch {
            try { management.moveCollectionItem(target, value, before); moving = null; refresh++ }
            catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { error = (failure as? LibraryFailure)?.reason ?: "INTERNAL" }
            finally { busy = false }
        }
    }
    ModalBottomSheet(onDismissRequest = { if (!busy) dismiss() }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().heightIn(max = 650.dp).padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.order), style = MaterialTheme.typography.headlineSmall)
            if (loading || busy) LinearProgressIndicator(Modifier.fillMaxWidth())
            error?.let {
                Text(failureText(it), color = MaterialTheme.colorScheme.error)
                TextButton(onClick = { refresh++ }, enabled = !busy) { Text(stringResource(R.string.retry)) }
            }
            moving?.let { value ->
                Text(value.title, style = MaterialTheme.typography.titleMedium)
                Row {
                    TextButton(onClick = { move(value, first) }, enabled = !busy && first?.id != value.id) { Text(stringResource(R.string.move_first)) }
                    TextButton(onClick = { move(value, null) }, enabled = !busy) { Text(stringResource(R.string.move_last)) }
                    TextButton(onClick = { moving = null }, enabled = !busy) { Text(stringResource(R.string.cancel)) }
                }
            }
            LazyColumn(Modifier.weight(1f, fill = false)) {
                items(items, key = LibraryItem::id) { value ->
                    ListItem(headlineContent = { Text(value.title) }, trailingContent = {
                        TextButton(modifier = Modifier.testTag("order-select:" + value.id),
                            onClick = { val source = moving; if (source == null) moving = value else move(source, value) },
                            enabled = !busy && !loading && value.id != moving?.id) {
                            Text(stringResource(if (moving == null) R.string.order else R.string.move_before))
                        }
                    })
                }
            }
            if (cursor != null) TextButton(onClick = {
                val next = cursor ?: return@TextButton
                loading = true
                scope.launch {
                    try {
                        val page = library.page("", next, id)
                        next.close(); cursor = page.next; items = page.items; error = null
                    } catch (cancel: CancellationException) { throw cancel }
                    catch (failure: Exception) { error = (failure as? LibraryFailure)?.reason ?: "INTERNAL" }
                    finally { loading = false }
                }
            }, enabled = !busy && !loading, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.load_more)) }
        }
    }
}
