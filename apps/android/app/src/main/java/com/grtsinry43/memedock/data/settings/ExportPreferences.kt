package com.grtsinry43.memedock.data.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.*
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.catch
import com.grtsinry43.memedock.data.settings.migrations.OutputDefaultsV0_1_0ToV0_1_3

private val Context.exportStore by preferencesDataStore(name = "export_preferences")
enum class ExportChoice { Original, CompatiblePng, WhiteBackground, SmallJpeg, BestCompatibility;
    companion object {
        fun applicable(animated: Boolean) = if (animated) listOf(BestCompatibility, Original)
            else listOf(BestCompatibility, Original, WhiteBackground, SmallJpeg)
    }
}

enum class OutputAction { Copy, Share }

data class OutputDefaults(
    val copyStatic: ExportChoice = ExportChoice.BestCompatibility,
    val copyAnimated: ExportChoice = ExportChoice.BestCompatibility,
    val shareStatic: ExportChoice = ExportChoice.BestCompatibility,
    val shareAnimated: ExportChoice = ExportChoice.BestCompatibility,
) {
    fun choice(action: OutputAction, animated: Boolean) = when (action) {
        OutputAction.Copy -> if (animated) copyAnimated else copyStatic
        OutputAction.Share -> if (animated) shareAnimated else shareStatic
    }
}

data class OutputPreferencesState(val defaults: OutputDefaults? = null, val failed: Boolean = false)

internal fun outputKey(action: OutputAction, animated: Boolean) =
    stringPreferencesKey("${action.name.lowercase()}.${if (animated) "animated" else "static"}")

class ExportPreferences(private val store: DataStore<Preferences>) {
    constructor(context: Context) : this(context.applicationContext.exportStore)
    val defaults = flow {
        store.edit { OutputDefaultsV0_1_0ToV0_1_3.migrate(it) }
        emitAll(store.data.map { value ->
            fun read(action: OutputAction, animated: Boolean): ExportChoice {
                val stored = value[outputKey(action, animated)]
                return ExportChoice.applicable(animated).firstOrNull { it.name == stored }
                    ?: throw java.io.IOException("Invalid output preference")
            }
            OutputDefaults(read(OutputAction.Copy, false), read(OutputAction.Copy, true),
                read(OutputAction.Share, false), read(OutputAction.Share, true))
        })
    }
    val state = defaults.map { OutputPreferencesState(it) }.catch { error ->
        if (error is java.io.IOException) emit(OutputPreferencesState(failed = true)) else throw error
    }
    suspend fun select(action: OutputAction, animated: Boolean, choice: ExportChoice) {
        require(choice in ExportChoice.applicable(animated))
        store.edit {
            OutputDefaultsV0_1_0ToV0_1_3.migrate(it)
            it[outputKey(action, animated)] = choice.name
        }
    }
}
