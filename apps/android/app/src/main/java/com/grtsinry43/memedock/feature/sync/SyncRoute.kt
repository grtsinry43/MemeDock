package com.grtsinry43.memedock.feature.sync

import androidx.compose.runtime.*
import androidx.lifecycle.compose.collectAsStateWithLifecycle

@Composable
fun SyncRoute(coordinator: SyncCoordinator, back: () -> Unit) {
    val state by coordinator.state.collectAsStateWithLifecycle()
    SyncScreen(state, back, coordinator::connect, coordinator::synchronizeNow,
        coordinator::setPaused, coordinator::disconnect, coordinator::setWifiOnly)
}
