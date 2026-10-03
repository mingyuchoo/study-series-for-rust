package app.stillnote

import androidx.lifecycle.SavedStateHandle
import app.stillnote.application.*
import app.stillnote.domain.*
import app.stillnote.presentation.JournalViewModel
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class JournalViewModelTest {
    private val dispatcher = StandardTestDispatcher()
    private val date = parseDate("2024-02-29")
    private var sequence = 0L
    private val journals =
        object : JournalRepository {
            var value = Journal()
            var loadError: String? = null
            var saveError: String? = null

            override fun load(): Journal {
                loadError?.let { throw RepositoryException(it) }
                return value
            }

            override fun save(journal: Journal) {
                saveError?.let { throw RepositoryException(it) }
                value = journal
            }
        }
    private val preferences =
        object : SettingsRepository {
            var value = Settings()
            var error: String? = null

            override fun load() = value

            override fun save(settings: Settings) {
                error?.let { throw RepositoryException(it) }
                value = settings
            }
        }

    @Before
    fun setUp() {
        Dispatchers.setMain(dispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun viewModel() =
        JournalViewModel(
            JournalService(journals, preferences, IdGenerator { UUID(0, ++sequence) }),
            SavedStateHandle(),
            dispatcher,
        )

    @Test
    fun failedSaveKeepsPublishedJournalAndDraftCallbackThenRetrySucceeds() =
        runTest(dispatcher) {
            val vm = viewModel()
            advanceUntilIdle()
            var successes = 0
            journals.saveError = "journal_save"
            val command = JournalCommand.AddEntry(date, Log.Daily, Kind.Task, "draft")
            vm.execute(command) { successes++ }
            advanceUntilIdle()
            assertEquals(Journal(), vm.state.value.journal)
            assertEquals("journal_save", vm.state.value.error)
            assertFalse(vm.state.value.busy)
            assertEquals(0, successes)
            journals.saveError = null
            vm.execute(command) { successes++ }
            advanceUntilIdle()
            assertEquals("draft", vm.state.value.journal.entries.single().text)
            assertNull(vm.state.value.error)
            assertEquals(1, successes)
        }

    @Test
    fun queuedCommandsUseLatestCommittedJournal() =
        runTest(dispatcher) {
            val vm = viewModel()
            advanceUntilIdle()
            repeat(12) {
                vm.execute(JournalCommand.AddEntry(date, Log.Daily, Kind.Note, "note $it"))
            }
            advanceUntilIdle()
            assertEquals(12, vm.state.value.journal.entries.size)
            assertEquals(journals.value, vm.state.value.journal)
            assertFalse(vm.state.value.busy)
        }

    @Test
    fun corruptJournalBlocksCommandsButSettingsRemainSelectable() =
        runTest(dispatcher) {
            journals.loadError = "journal_load"
            val vm = viewModel()
            advanceUntilIdle()
            assertTrue(vm.state.value.blocked)
            var success = false
            vm.execute(JournalCommand.AddCollection("books")) { success = true }
            preferences.error = "settings_protected"
            val selected = Settings(Language.English, ThemeMode.Dark)
            vm.settings(selected)
            advanceUntilIdle()
            assertFalse(success)
            assertTrue(vm.state.value.journal.collections.isEmpty())
            assertEquals(selected, vm.state.value.settings)
            assertEquals(Settings(), preferences.value)
            assertEquals("settings_protected", vm.state.value.error)
        }
}
