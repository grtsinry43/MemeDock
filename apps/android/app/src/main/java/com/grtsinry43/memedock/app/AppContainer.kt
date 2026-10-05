package com.grtsinry43.memedock.app

import android.content.Context
import coil3.ImageLoader
import coil3.memory.MemoryCache
import com.grtsinry43.memedock.data.library.RustLibraryRepository
import com.grtsinry43.memedock.feature.importing.ImportCoordinator
import com.grtsinry43.memedock.platform.importing.AndroidImportGateway
import kotlinx.coroutines.*

class AppContainer(context: Context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val session = LibrarySession(context, scope)
    val library = RustLibraryRepository(session)
    val imports = ImportCoordinator(library, AndroidImportGateway(context.contentResolver), scope)
    // Core owns disk thumbnails. Coil owns only a bounded display memory cache.
    val imageLoader = ImageLoader.Builder(context)
        .memoryCache { MemoryCache.Builder().maxSizeBytes(24L * 1024 * 1024).build() }
        .diskCache(null)
        .build()
}
