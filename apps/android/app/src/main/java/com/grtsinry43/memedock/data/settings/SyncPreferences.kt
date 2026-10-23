package com.grtsinry43.memedock.data.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.*
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.map

private val Context.syncStore by preferencesDataStore(name = "sync_preferences")

class SyncPreferences(private val store: DataStore<Preferences>) {
    constructor(context: Context) : this(context.applicationContext.syncStore)
    private val wifi = booleanPreferencesKey("wifi_only")
    val wifiOnly = store.data.map { it[wifi] ?: false }
    suspend fun setWifiOnly(value: Boolean) { store.edit { it[wifi] = value } }
}
