package com.grtsinry43.memedock.feature

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import com.grtsinry43.memedock.data.settings.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test
import java.nio.file.Files

class AppearancePreferencesTest {
    @Test fun themeChoiceSurvivesClosingAndReopeningTheRealStore() = runBlocking {
        val directory = Files.createTempDirectory("memedock-appearance").toFile()
        val file = directory.resolve("appearance.preferences_pb")
        suspend fun withStore(block: suspend (AppearancePreferences) -> Unit) {
            val job = SupervisorJob()
            val store = PreferenceDataStoreFactory.create(scope = CoroutineScope(job + Dispatchers.IO), produceFile = { file })
            try { block(AppearancePreferences(store)) } finally { job.cancelAndJoin() }
        }
        try {
            withStore { assertEquals(ThemeMode.System, it.mode.first()); it.select(ThemeMode.Dark) }
            withStore { assertEquals(ThemeMode.Dark, it.mode.first()); it.select(ThemeMode.Light) }
            withStore { assertEquals(ThemeMode.Light, it.mode.first()); it.select(ThemeMode.System) }
            withStore { assertEquals(ThemeMode.System, it.mode.first()) }
        } finally { directory.deleteRecursively() }
    }
}
