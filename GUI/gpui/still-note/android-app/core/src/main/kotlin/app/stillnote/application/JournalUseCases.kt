package app.stillnote.application

import app.stillnote.domain.Journal
import app.stillnote.domain.Settings

data class LoadedJournal(
    val journal: Journal = Journal(),
    val settings: Settings = Settings(),
    val blocked: Boolean = false,
    val error: String? = null,
)

/** Inbound port: callers need no knowledge of repository or identity adapters. */
interface JournalUseCases {
    fun load(): LoadedJournal

    fun execute(journal: Journal, command: JournalCommand): Journal

    fun saveSettings(settings: Settings)
}
