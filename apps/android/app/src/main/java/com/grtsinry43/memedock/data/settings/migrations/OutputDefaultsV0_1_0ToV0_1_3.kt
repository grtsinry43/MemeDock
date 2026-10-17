package com.grtsinry43.memedock.data.settings.migrations

import androidx.datastore.preferences.core.MutablePreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import com.grtsinry43.memedock.data.settings.ExportChoice
import com.grtsinry43.memedock.data.settings.OutputAction
import com.grtsinry43.memedock.data.settings.outputKey

/** The 0.1.0–0.1.2 setting shape upgrades atomically. Legacy keys remain intact. */
internal object OutputDefaultsV0_1_0ToV0_1_3 {
    fun migrate(value: MutablePreferences) {
        val legacy = value[stringPreferencesKey("last_preset")]
        for (action in OutputAction.entries) for (animated in listOf(false, true)) {
            val key = outputKey(action, animated)
            if (value[key] != null) continue
            val migrated = when (legacy) {
                "Original" -> ExportChoice.Original
                "WhiteBackground", "SmallJpeg" -> if (animated) ExportChoice.Original else ExportChoice.valueOf(legacy)
                else -> ExportChoice.BestCompatibility
            }
            value[key] = migrated.name
        }
    }
}
