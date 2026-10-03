package app.stillnote.presentation

import app.stillnote.application.JournalCommand
import app.stillnote.domain.Journal
import app.stillnote.domain.Settings

/** Screen events. The committed journal is supplied only after a successful save. */
interface JournalScreenActions {
    fun execute(command: JournalCommand, onSuccess: (Journal) -> Unit = {})

    fun settings(settings: Settings)

    fun report(error: String)

    fun dismissError()
}

/** Framework-independent access to restorable navigation and editor values. */
interface ScreenMemory {
    fun remember(key: String, value: String)

    fun recalled(key: String, default: String = ""): String
}
