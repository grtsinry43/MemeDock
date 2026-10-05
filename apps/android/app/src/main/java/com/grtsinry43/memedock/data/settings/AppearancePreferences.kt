package com.grtsinry43.memedock.data.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.*
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.map

private val Context.appearanceStore by preferencesDataStore(name = "appearance")
enum class ThemeMode { System, Light, Dark }

class AppearancePreferences(private val store: DataStore<Preferences>) {
    constructor(context: Context) : this(context.applicationContext.appearanceStore)
    private val key = stringPreferencesKey("theme_mode")
    val mode = store.data.map { preferences ->
        ThemeMode.entries.firstOrNull { it.name == preferences[key] } ?: ThemeMode.System
    }
    suspend fun select(mode: ThemeMode) { store.edit { it[key] = mode.name } }
}
