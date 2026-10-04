package app.stillnote.presentation

import app.stillnote.domain.Status
import java.util.UUID

/** A record component emits intent without knowing storage, navigation, or editor state. */
sealed interface EntryIntent {
    val id: UUID

    data class Edit(override val id: UUID) : EntryIntent

    data class ToggleImportant(override val id: UUID) : EntryIntent

    data class SetStatus(override val id: UUID, val status: Status) : EntryIntent

    data class Migrate(override val id: UUID) : EntryIntent

    data class Open(override val id: UUID) : EntryIntent
}
