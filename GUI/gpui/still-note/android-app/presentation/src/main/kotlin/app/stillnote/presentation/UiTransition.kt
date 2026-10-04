package app.stillnote.presentation

import app.stillnote.application.LoadedJournal
import app.stillnote.application.RepositoryException
import app.stillnote.domain.*

/** Pure state changes; coroutine scheduling and persistence stay in the Android adapter. */
sealed interface UiTransition {
    data class Loaded(val result: LoadedJournal) : UiTransition

    data object CommandStarted : UiTransition

    data class CommandCommitted(val journal: Journal) : UiTransition

    data class CommandFailed(val code: String) : UiTransition

    data object CommandCancelled : UiTransition

    data class SettingsSelected(val settings: Settings) : UiTransition

    data class ErrorReported(val code: String) : UiTransition

    data object ErrorDismissed : UiTransition
}

fun UiState.reduce(transition: UiTransition): UiState =
    when (transition) {
        is UiTransition.Loaded ->
            UiState(
                journal = transition.result.journal,
                settings = transition.result.settings,
                loading = false,
                blocked = transition.result.blocked,
                error = transition.result.error,
            )
        UiTransition.CommandStarted -> copy(busy = true)
        is UiTransition.CommandCommitted ->
            copy(journal = transition.journal, busy = false, error = null)
        is UiTransition.CommandFailed -> copy(busy = false, error = transition.code)
        UiTransition.CommandCancelled -> copy(busy = false)
        is UiTransition.SettingsSelected -> copy(settings = transition.settings)
        is UiTransition.ErrorReported -> copy(error = transition.code)
        UiTransition.ErrorDismissed -> copy(error = null)
    }

fun commandErrorCode(error: Exception): String =
    when (error) {
        is DomainException -> error.code
        is RepositoryException -> error.code
        else -> "journal_save"
    }
