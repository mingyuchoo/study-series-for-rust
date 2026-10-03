package app.stillnote.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.stillnote.application.JournalCommand
import app.stillnote.domain.Journal
import app.stillnote.domain.Settings
import app.stillnote.presentation.JournalScreenActions
import app.stillnote.presentation.JournalViewModel
import app.stillnote.presentation.ScreenMemory
import java.time.LocalDate

/** Android route owns lifecycle collection, saved-state and clock effects. */
@Composable
fun StillnoteApp(vm: JournalViewModel) {
    val state by vm.state.collectAsStateWithLifecycle()
    val actions =
        remember(vm) {
            object : JournalScreenActions {
                override fun execute(command: JournalCommand, onSuccess: (Journal) -> Unit) {
                    vm.execute(command) { onSuccess(vm.state.value.journal) }
                }

                override fun settings(settings: Settings) = vm.settings(settings)

                override fun report(error: String) = vm.report(error)

                override fun dismissError() = vm.dismissError()
            }
        }
    val memory =
        remember(vm) {
            object : ScreenMemory {
                override fun remember(key: String, value: String) = vm.remember(key, value)

                override fun recalled(key: String, default: String) = vm.recalled(key, default)
            }
        }
    StillnoteScreen(state, actions, memory, LocalDate::now)
}
