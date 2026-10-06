package com.grtsinry43.memedock.data.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.*
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.map

private val Context.exportStore by preferencesDataStore(name = "export_preferences")
enum class ExportChoice { Original, CompatiblePng, WhiteBackground, SmallJpeg }

class ExportPreferences(private val store: DataStore<Preferences>) {
    constructor(context: Context) : this(context.applicationContext.exportStore)
    private val key = stringPreferencesKey("last_preset")
    val choice = store.data.map { value ->
        ExportChoice.entries.firstOrNull { it.name == value[key] } ?: ExportChoice.Original
    }
    suspend fun select(choice: ExportChoice) { store.edit { it[key] = choice.name } }
}
