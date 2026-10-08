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
    val language = com.grtsinry43.memedock.data.settings.LanguagePreferences(context)
    val exportPreferences = com.grtsinry43.memedock.data.settings.ExportPreferences(context)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val session = LibrarySession(context, scope)
    val libraryEpoch = session.epoch
    val backups = com.grtsinry43.memedock.feature.backup.BackupCoordinator(
        com.grtsinry43.memedock.data.library.BackupRepository(session),
        com.grtsinry43.memedock.platform.backup.AndroidBackupGateway(context.contentResolver), scope)
    val library = RustLibraryRepository(session)
    val telegram = com.grtsinry43.memedock.data.telegram.RustTelegramRepository(session)
    val telegramTokens = com.grtsinry43.memedock.platform.credentials.AndroidTelegramTokenStore(context)
    val imports = ImportCoordinator(library, AndroidImportGateway(context.contentResolver), scope)
    val shares = AndroidShareGateway()
    val clipboard = com.grtsinry43.memedock.platform.clipboard.ClipboardCoordinator(library,
        com.grtsinry43.memedock.platform.clipboard.AndroidClipboardGateway(context), scope)
    val saves = com.grtsinry43.memedock.platform.saving.SaveCoordinator(library,
        com.grtsinry43.memedock.platform.saving.AndroidSaveGateway(context.contentResolver), scope)
    // Core owns disk thumbnails. Coil owns only a bounded display memory cache.
    val imageLoader = ImageLoader.Builder(context)
        .components { add(AnimatedImageDecoder.Factory()) }
        .memoryCache { MemoryCache.Builder().maxSizeBytes(24L * 1024 * 1024).build() }
        .diskCache(null)
        .build()
}
