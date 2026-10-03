package app.stillnote

import app.stillnote.application.*
import app.stillnote.domain.*
import java.util.UUID
import org.junit.Assert.*
import org.junit.Test

class JournalServiceTest {
    private val date = parseDate("2024-02-29")
    private val id = UUID(0, 1)

    private class Journals : JournalRepository {
        var value = Journal()
        var loadError: String? = null
        var saveError: String? = null
        var saves = 0

        override fun load(): Journal {
            loadError?.let { throw RepositoryException(it) }
            return value
        }

        override fun save(journal: Journal) {
            saveError?.let { throw RepositoryException(it) }
            value = journal
            saves++
        }
    }

    private class Preferences : SettingsRepository {
        var value = Settings(Language.English, ThemeMode.Dark)
        var loadError: String? = null
        var saveError: String? = null

        override fun load(): Settings {
            loadError?.let { throw RepositoryException(it) }
            return value
        }

        override fun save(settings: Settings) {
            saveError?.let { throw RepositoryException(it) }
            value = settings
        }
    }

    @Test
    fun pureCommandsAreRepeatableAndLeaveInputUnchanged() {
        val before = Journal()
        val command = JournalCommand.AddEntry(date, Log.Daily, Kind.Task, " task ")
        val after = before.apply(command, id)
        assertEquals(after, before.apply(command, id))
        assertTrue(before.entries.isEmpty())
        assertEquals("task", after.entry(id).text)
        val migrated =
            after.apply(JournalCommand.Migrate(id, date.plusDays(1), Log.Future), UUID(0, 2))
        assertEquals(Status.Scheduled, migrated.entry(id).status)
        assertEquals(id, migrated.entry(UUID(0, 2)).migratedFrom)
        assertEquals(Status.Open, after.entry(id).status)
    }

    @Test
    fun saveFailureDoesNotPublishCandidateAndCanBeRetried() {
        val journals = Journals()
        val service = JournalService(journals, Preferences(), IdGenerator { id })
        val before = service.load().journal
        val command = JournalCommand.AddEntry(date, Log.Daily, Kind.Note, "draft")
        journals.saveError = "journal_conflict"
        val error =
            assertThrows(RepositoryException::class.java) { service.execute(before, command) }
        assertEquals("journal_conflict", error.code)
        assertEquals(before, journals.value)
        journals.saveError = null
        assertEquals(before.apply(command, id), service.execute(before, command))
        assertEquals(1, journals.saves)
    }

    @Test
    fun invalidCommandNeverSavesAndUpdatesDoNotGenerateIds() {
        val journals = Journals()
        journals.value = Journal().addEntry(date, Log.Daily, Kind.Task, "task", id)
        var generated = 0
        val service =
            JournalService(
                journals,
                Preferences(),
                IdGenerator {
                    generated++
                    UUID(0, 2)
                },
            )
        val initial = service.load().journal
        assertThrows(DomainException::class.java) {
            service.execute(initial, JournalCommand.EditEntry(id, " "))
        }
        assertEquals(0, journals.saves)
        val edited = service.execute(initial, JournalCommand.EditEntry(id, "edited"))
        val important = service.execute(edited, JournalCommand.ToggleImportant(id))
        val completed = service.execute(important, JournalCommand.SetStatus(id, Status.Complete))
        assertTrue(completed.entry(id).important)
        assertEquals("edited", completed.entry(id).text)
        assertEquals(Status.Complete, completed.entry(id).status)
        assertEquals(0, generated)
    }

    @Test
    fun settingsLoadFailureAllowsJournalAndJournalFailureTakesPrecedence() {
        val journals = Journals()
        journals.value = Journal().addCollection("books", id)
        val preferences = Preferences().apply { loadError = "settings_load" }
        val service = JournalService(journals, preferences, IdGenerator { id })
        val loaded = service.load()
        assertEquals(journals.value, loaded.journal)
        assertEquals(Settings(), loaded.settings)
        assertFalse(loaded.blocked)
        assertEquals("settings_load", loaded.error)
        journals.loadError = "journal_load"
        assertEquals(LoadedJournal(blocked = true, error = "journal_load"), service.load())
    }

    @Test
    fun corruptJournalPreservesLoadedSettingsAndSettingsFailureIsReported() {
        val journals = Journals().apply { loadError = "journal_load" }
        val preferences = Preferences()
        val service = JournalService(journals, preferences, IdGenerator { id })
        assertEquals(
            LoadedJournal(settings = preferences.value, blocked = true, error = "journal_load"),
            service.load(),
        )
        preferences.saveError = "settings_protected"
        val before = preferences.value
        assertThrows(RepositoryException::class.java) { service.saveSettings(Settings()) }
        assertEquals(before, preferences.value)
    }
}
