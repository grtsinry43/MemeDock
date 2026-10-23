package com.grtsinry43.memedock.feature.trash

import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.ui.components.*
import com.grtsinry43.memedock.ui.failureTextRes

@Composable
fun PurgeConfirmation(coordinator: TrashCoordinator) {
    val state by coordinator.state.collectAsStateWithLifecycle()
    val messages = LocalMemeDockMessages.current
    val resources = LocalResources.current
    LaunchedEffect(state.report, state.error, state.cleanupDone, state.empty) {
        val report = state.report
        val error = state.error
        val message = when {
            report != null -> MemeDockMessage(resources.getString(R.string.purge_result, report.succeeded.size, report.failed) +
                (report.error?.let { "\n" + resources.getString(failureTextRes(it)) } ?: ""),
                if (report.failed > 0) MemeDockMessageType.Warning else MemeDockMessageType.Success)
            error != null -> MemeDockMessage(resources.getString(failureTextRes(error)), MemeDockMessageType.Error)
            state.cleanupDone -> MemeDockMessage(resources.getString(R.string.purge_cleanup_done), MemeDockMessageType.Success)
            state.empty -> MemeDockMessage(resources.getString(R.string.trash_empty), MemeDockMessageType.Success)
            else -> return@LaunchedEffect
        }
        // Keep acknowledgement after the suspending delivery so this effect cannot cancel its message.
        messages.showMemeDockMessage(message)
        coordinator.acknowledge()
    }
    val request = state.request ?: return
    AlertDialog(onDismissRequest = coordinator::dismiss, title = { Text(stringResource(R.string.purge_title)) },
        text = {
            Text(if (state.busy) stringResource(R.string.purge_progress, state.completed, state.total)
                else stringResource(R.string.purge_hint, request.targets.size))
        }, confirmButton = {
            TextButton(coordinator::confirm, enabled = !state.busy, modifier = Modifier.testTag("purge-confirm")) {
                Text(stringResource(R.string.purge_title), color = MaterialTheme.colorScheme.error)
            }
        }, dismissButton = {
            TextButton(coordinator::dismiss, enabled = !state.busy) { Text(stringResource(R.string.cancel)) }
        })
}
