package app.stillnote.application

import app.stillnote.domain.*

/** Effectful use cases depend only on ports. Scheduling belongs to the caller. */
class JournalService(
    private val journals: JournalRepository,
    private val preferences: SettingsRepository,
    private val ids: IdGenerator,
) : JournalUseCases {
    override fun load(): LoadedJournal {
        var error: String? = null
        val settings =
            try {
                preferences.load()
            } catch (e: RepositoryException) {
                error = e.code
                Settings()
            }
        return try {
            LoadedJournal(journals.load(), settings, error = error)
        } catch (e: RepositoryException) {
            LoadedJournal(settings = settings, blocked = true, error = e.code)
        }
    }

    override fun execute(journal: Journal, command: JournalCommand): Journal {
        val next =
            journal.apply(
                command,
                when (command) {
                    is JournalCommand.AddEntry,
                    is JournalCommand.AddCollection,
                    is JournalCommand.Migrate -> ids.next()
                    else -> null
                },
            )
        journals.save(next)
        return next
    }

    override fun saveSettings(settings: Settings) = preferences.save(settings)
}
