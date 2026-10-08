package com.grtsinry43.memedock.feature.settings

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.*
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.grtsinry43.memedock.app.AppContainer

@Composable
fun SettingsRoute(container: AppContainer, contentPadding: PaddingValues, trash: () -> Unit, backup: () -> Unit, licenses: () -> Unit) {
    val factory = remember(container) { object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T =
            modelClass.cast(SettingsViewModel(container.appearance, container.language, container.library))!!
    } }
    val model: SettingsViewModel = viewModel(factory = factory)
    val state by model.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val version = remember(context) { context.versionName() }
    LaunchedEffect(model) { model.refresh() }
    SettingsScreen(state, version, model::select, model::selectLanguage, model::retry, trash, backup, licenses, contentPadding)
}

private fun Context.versionName(): String {
    val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU)
        packageManager.getPackageInfo(packageName, PackageManager.PackageInfoFlags.of(0))
    else @Suppress("DEPRECATION") packageManager.getPackageInfo(packageName, 0)
    return info.versionName.orEmpty()
}
