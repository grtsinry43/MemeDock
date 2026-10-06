package com.grtsinry43.memedock.feature

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import com.grtsinry43.memedock.data.settings.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test
import java.nio.file.Files

class ExportPreferencesTest {
    @Test fun presetSurvivesReopeningWithoutRememberingAnimationConsent() = runBlocking {
        val directory = Files.createTempDirectory("memedock-export-preferences").toFile()
        val file = directory.resolve("export.preferences_pb")
        suspend fun withStore(block: suspend (ExportPreferences) -> Unit) {
            val job = SupervisorJob()
            val store = PreferenceDataStoreFactory.create(scope = CoroutineScope(job + Dispatchers.IO), produceFile = { file })
            try { block(ExportPreferences(store)) } finally { job.cancelAndJoin() }
        }
        try {
            withStore { assertEquals(ExportChoice.Original, it.choice.first()) }
            ExportChoice.entries.forEach { choice ->
                withStore { it.select(choice) }
                withStore { assertEquals(choice, it.choice.first()) }
            }
        } finally { assertTrue(directory.deleteRecursively()) }
    }
}
