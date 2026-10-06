package com.grtsinry43.memedock.data.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.*
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.map

private val Context.languageStore by preferencesDataStore(name = "language")
enum class LanguageMode { System, Chinese, English }

class LanguagePreferences(private val store: DataStore<Preferences>) {
    constructor(context: Context) : this(context.applicationContext.languageStore)
    private val key = stringPreferencesKey("language_mode")
    val mode = store.data.map { preferences ->
        LanguageMode.entries.firstOrNull { it.name == preferences[key] } ?: LanguageMode.System
    }
    suspend fun select(mode: LanguageMode) { store.edit { it[key] = mode.name } }
}
