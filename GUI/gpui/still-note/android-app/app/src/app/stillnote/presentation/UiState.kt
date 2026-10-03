package app.stillnote.presentation

import app.stillnote.domain.Journal
import app.stillnote.domain.Settings

data class UiState(
    val journal: Journal = Journal(),
    val settings: Settings = Settings(),
    val loading: Boolean = true,
    val blocked: Boolean = false,
    val busy: Boolean = false,
    val error: String? = null,
)
