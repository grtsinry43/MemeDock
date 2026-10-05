package com.grtsinry43.memedock.app

import android.content.Context
import coil3.ImageLoader
import coil3.memory.MemoryCache
import coil3.gif.AnimatedImageDecoder
import com.grtsinry43.memedock.platform.sharing.AndroidShareGateway
import com.grtsinry43.memedock.data.library.RustLibraryRepository
import com.grtsinry43.memedock.feature.importing.ImportCoordinator
import com.grtsinry43.memedock.platform.importing.AndroidImportGateway
import kotlinx.coroutines.*

class AppContainer(context: Context) {
    val appearance = com.grtsinry43.memedock.data.settings.AppearancePreferences(context)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val session = LibrarySession(context, scope)
    val library = RustLibraryRepository(session)
    val imports = ImportCoordinator(library, AndroidImportGateway(context.contentResolver), scope)
    val shares = AndroidShareGateway()
    // Core owns disk thumbnails. Coil owns only a bounded display memory cache.
    val imageLoader = ImageLoader.Builder(context)
        .components { add(AnimatedImageDecoder.Factory()) }
        .memoryCache { MemoryCache.Builder().maxSizeBytes(24L * 1024 * 1024).build() }
        .diskCache(null)
        .build()
}
