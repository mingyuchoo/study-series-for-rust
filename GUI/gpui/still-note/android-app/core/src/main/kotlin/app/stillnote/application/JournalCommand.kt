package app.stillnote.application

import app.stillnote.domain.*
import java.time.LocalDate
import java.util.UUID

sealed interface JournalCommand {
    data class AddEntry(val date: LocalDate, val log: Log, val kind: Kind, val text: String) :
        JournalCommand

    data class AddCollection(val name: String) : JournalCommand

    data class EditEntry(val id: UUID, val text: String) : JournalCommand

    data class SetStatus(val id: UUID, val status: Status) : JournalCommand

    data class ToggleImportant(val id: UUID) : JournalCommand

    data class Migrate(val id: UUID, val date: LocalDate, val log: Log) : JournalCommand
}

/** Pure transition: identity is supplied by the caller, never generated here. */
fun Journal.apply(command: JournalCommand, newId: UUID? = null): Journal =
    when (command) {
        is JournalCommand.AddEntry ->
            addEntry(command.date, command.log, command.kind, command.text, requireNotNull(newId))
        is JournalCommand.AddCollection -> addCollection(command.name, requireNotNull(newId))
        is JournalCommand.EditEntry -> editEntry(command.id, command.text)
        is JournalCommand.SetStatus -> setStatus(command.id, command.status)
        is JournalCommand.ToggleImportant -> toggleImportant(command.id)
        is JournalCommand.Migrate ->
            migrate(command.id, command.date, command.log, requireNotNull(newId))
    }
