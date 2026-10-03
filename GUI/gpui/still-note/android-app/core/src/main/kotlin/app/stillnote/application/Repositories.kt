package app.stillnote.application

import app.stillnote.domain.Journal
import app.stillnote.domain.Settings
import java.util.UUID

interface JournalRepository {
    fun load(): Journal

    fun save(journal: Journal)
}

interface SettingsRepository {
    fun load(): Settings

    fun save(settings: Settings)
}

fun interface IdGenerator {
    fun next(): UUID
}

class RepositoryException(val code: String, cause: Throwable? = null) :
    IllegalStateException(code, cause)
