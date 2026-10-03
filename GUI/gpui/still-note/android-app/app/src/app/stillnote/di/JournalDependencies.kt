package app.stillnote.di

import app.stillnote.application.IdGenerator
import app.stillnote.application.JournalService
import app.stillnote.application.JournalUseCases
import app.stillnote.data.JournalStore
import app.stillnote.data.SettingsStore
import java.io.File
import java.util.UUID

/** Composition root for file-backed production dependencies. */
object JournalDependencies {
    fun create(directory: File): JournalUseCases =
        JournalService(
            JournalStore(File(directory, "journal.json")),
            SettingsStore(File(directory, "settings.json")),
            IdGenerator { UUID.randomUUID() },
        )
}
