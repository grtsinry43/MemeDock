package com.grtsinry43.memedock.app

import android.content.Context
import com.grtsinry43.memedock.bridge.asFlow
import com.grtsinry43.memedock.bridge.generated.LibraryConfiguration
import com.grtsinry43.memedock.bridge.generated.LibraryHandle
import com.grtsinry43.memedock.bridge.generated.Notification
import com.grtsinry43.memedock.bridge.generated.defaultResourceConfiguration
import com.grtsinry43.memedock.bridge.generated.openLibrary
import com.grtsinry43.memedock.data.library.LibraryChange
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.io.File

/** Process-owned. An Activity or page never opens another native instance. */
class LibrarySession(context: Context, private val scope: CoroutineScope) {
    private val data = File(context.filesDir, "library")
    private val cache = File(context.cacheDir, "library")
    private val share = File(context.filesDir, "share")
    private sealed interface Connection {
        data object Opening : Connection
        data class Ready(val library: LibraryHandle) : Connection
        data class Failed(val error: Exception) : Connection
    }
    private val connection = MutableStateFlow<Connection>(Connection.Opening)
    private val events = MutableSharedFlow<LibraryChange>(extraBufferCapacity = 1)
    val changes: SharedFlow<LibraryChange> = events.asSharedFlow()
    private var opening: Job? = null

    init { retryOpen() }

    fun retryOpen() {
        if (opening?.isActive == true || connection.value is Connection.Ready) return
        connection.value = Connection.Opening
        opening = scope.launch {
            try {
                val library = withContext(Dispatchers.IO) {
                    openLibrary(LibraryConfiguration(data.path, cache.path, share.path, defaultResourceConfiguration()))
                }
                // Subscribe before exposing Ready, so the first query cannot miss a write.
                try {
                    val subscription = library.subscribe()
                    connection.value = Connection.Ready(library)
                    events.emit(LibraryChange.Reload)
                    subscription.asFlow().collect { event ->
                        events.emit(when (event) {
                            is Notification.ThumbnailChanged -> LibraryChange.Thumbnail(event.stickerId)
                            is Notification.StickerChanged, is Notification.UsageChanged -> LibraryChange.Content
                            else -> LibraryChange.Reload
                        })
                    }
                    throw com.grtsinry43.memedock.data.library.LibraryFailure("CLOSED")
                } finally {
                    withContext(NonCancellable + Dispatchers.IO) {
                        try { library.shutdown() } finally { library.close() }
                    }
                }
            } catch (cancel: CancellationException) {
                throw cancel
            } catch (error: Exception) {
                connection.value = Connection.Failed(error)
                events.emit(LibraryChange.Reload)
            }
        }
    }

    internal suspend fun library(): LibraryHandle = when (val state = connection.first { it !is Connection.Opening }) {
        is Connection.Ready -> state.library
        is Connection.Failed -> throw state.error
        Connection.Opening -> error("Unreachable opening state")
    }
}
