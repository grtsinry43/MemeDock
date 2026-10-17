package com.grtsinry43.memedock.feature

import androidx.datastore.preferences.core.*
import com.grtsinry43.memedock.data.settings.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test
import java.nio.file.Files
import java.io.File

class ExportPreferencesTest {
    private suspend fun withStore(file: File, block: suspend (ExportPreferences, androidx.datastore.core.DataStore<Preferences>) -> Unit) {
        val job = SupervisorJob()
        val store = PreferenceDataStoreFactory.create(scope = CoroutineScope(job + Dispatchers.IO), produceFile = { file })
        try { block(ExportPreferences(store), store) } finally { job.cancelAndJoin() }
    }
    @Test fun presetSurvivesReopeningWithoutRememberingAnimationConsent() = runBlocking {
        val directory = Files.createTempDirectory("memedock-export-preferences").toFile()
        val file = directory.resolve("export.preferences_pb")
        try {
            withStore(file) { settings, _ ->
                assertEquals(OutputDefaults(), settings.defaults.first())
                settings.select(OutputAction.Copy, false, ExportChoice.WhiteBackground)
                settings.select(OutputAction.Share, true, ExportChoice.Original)
            }
            withStore(file) { settings, _ ->
                assertEquals(OutputDefaults(copyStatic = ExportChoice.WhiteBackground, shareAnimated = ExportChoice.Original),
                    settings.defaults.first())
            }
        } finally { assertTrue(directory.deleteRecursively()) }
    }

    @Test fun releasedLegacyChoicesMigrateWithoutOverwritingNewChoices() = runBlocking {
        for ((legacy, still, animated) in listOf(
            Triple("Original", ExportChoice.Original, ExportChoice.Original),
            Triple("CompatiblePng", ExportChoice.BestCompatibility, ExportChoice.BestCompatibility),
            Triple("WhiteBackground", ExportChoice.WhiteBackground, ExportChoice.Original),
            Triple("SmallJpeg", ExportChoice.SmallJpeg, ExportChoice.Original),
        )) {
            val directory = Files.createTempDirectory("memedock-output-migration").toFile()
            val file = directory.resolve("export.preferences_pb")
            try {
                withStore(file) { settings, store ->
                    store.edit {
                        it[stringPreferencesKey("last_preset")] = legacy
                        it[stringPreferencesKey("copy.static")] = "SmallJpeg"
                    }
                    assertEquals(OutputDefaults(ExportChoice.SmallJpeg, animated, still, animated), settings.defaults.first())
                    assertEquals(legacy, store.data.first()[stringPreferencesKey("last_preset")])
                    settings.select(OutputAction.Share, true, ExportChoice.BestCompatibility)
                }
                withStore(file) { settings, _ ->
                    assertEquals(OutputDefaults(ExportChoice.SmallJpeg, animated, still, ExportChoice.BestCompatibility),
                        settings.defaults.first())
                }
            } finally { assertTrue(directory.deleteRecursively()) }
        }
    }

    @Test fun invalidAnimatedPresetCannotReplacePersistedChoice() = runBlocking {
        val directory = Files.createTempDirectory("memedock-output-validation").toFile()
        try {
            withStore(directory.resolve("export.preferences_pb")) { settings, store ->
                settings.select(OutputAction.Copy, true, ExportChoice.Original)
                try {
                    settings.select(OutputAction.Copy, true, ExportChoice.WhiteBackground)
                    fail("Expected invalid animated preset")
                } catch (_: IllegalArgumentException) { }
                assertEquals(ExportChoice.Original, settings.defaults.first().copyAnimated)
                store.edit { it[stringPreferencesKey("share.animated")] = "future-format" }
                try {
                    settings.defaults.first()
                    fail("Expected unsupported persisted preset")
                } catch (_: java.io.IOException) { }
                assertEquals("future-format", store.data.first()[stringPreferencesKey("share.animated")])
            }
        } finally { assertTrue(directory.deleteRecursively()) }
    }
}
