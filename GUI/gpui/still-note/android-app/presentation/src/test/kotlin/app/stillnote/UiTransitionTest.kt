package app.stillnote

import app.stillnote.application.*
import app.stillnote.domain.*
import app.stillnote.presentation.*
import java.util.UUID
import org.junit.Assert.*
import org.junit.Test

class UiTransitionTest {
    private val journal = Journal().addCollection("Books", UUID(0, 1))

    @Test
    fun failureRetainsCommittedJournalAndSuccessPublishesOnlyTheSuppliedResult() {
        val initial = UiState(journal = journal, loading = false)
        val pending = initial.reduce(UiTransition.CommandStarted)
        val failed = pending.reduce(UiTransition.CommandFailed("journal_conflict"))
        assertEquals(journal, failed.journal)
        assertEquals("journal_conflict", failed.error)
        assertFalse(failed.busy)
        val next = journal.addCollection("Work", UUID(0, 2))
        val committed =
            failed.reduce(UiTransition.CommandStarted).reduce(UiTransition.CommandCommitted(next))
        assertEquals(next, committed.journal)
        assertNull(committed.error)
        assertFalse(committed.busy)
        assertEquals(journal, initial.journal)
    }

    @Test
    fun cancellationAndSettingsSelectionRetainRecoveryErrorsAndTheJournal() {
        val pending =
            UiState(
                journal = journal,
                loading = false,
                blocked = true,
                busy = true,
                error = "journal_load",
            )
        val cancelled = pending.reduce(UiTransition.CommandCancelled)
        assertEquals(pending.copy(busy = false), cancelled)
        val selected = Settings(Language.English, ThemeMode.Dark)
        val changed = cancelled.reduce(UiTransition.SettingsSelected(selected))
        assertEquals(selected, changed.settings)
        assertEquals(journal, changed.journal)
        assertTrue(changed.blocked)
        assertEquals("journal_load", changed.error)
    }

    @Test
    fun loadingFailureAndErrorDismissalDoNotRemoveTheWriteBlock() {
        val loaded =
            UiState()
                .reduce(UiTransition.Loaded(LoadedJournal(blocked = true, error = "journal_load")))
        assertFalse(loaded.loading)
        assertTrue(loaded.blocked)
        val dismissed = loaded.reduce(UiTransition.ErrorDismissed)
        assertNull(dismissed.error)
        assertTrue(dismissed.blocked)
        assertEquals("date", dismissed.reduce(UiTransition.ErrorReported("date")).error)
    }

    @Test
    fun commandErrorsKeepDomainAndRepositoryCodesAndMapUnexpectedFailures() {
        assertEquals("text_empty", commandErrorCode(DomainException("text_empty")))
        assertEquals("journal_conflict", commandErrorCode(RepositoryException("journal_conflict")))
        assertEquals("journal_save", commandErrorCode(IllegalStateException("unexpected")))
    }
}
