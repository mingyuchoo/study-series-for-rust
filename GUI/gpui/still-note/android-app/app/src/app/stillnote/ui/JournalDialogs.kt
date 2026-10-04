package app.stillnote.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import app.stillnote.presentation.JournalPage
import app.stillnote.presentation.JournalStrings

@Composable
internal fun EditEntryDialog(
    text: String,
    busy: Boolean,
    error: String?,
    strings: JournalStrings,
    onText: (String) -> Unit,
    onSave: () -> Unit,
    onClose: () -> Unit,
) {
    AlertDialog(
        containerColor = MaterialTheme.colorScheme.surface,
        titleContentColor = MaterialTheme.colorScheme.onBackground,
        textContentColor = MaterialTheme.colorScheme.onSurface,
        shape = RoundedCornerShape(12.dp),
        tonalElevation = 0.dp,
        onDismissRequest = { if (!busy) onClose() },
        title = { Text(strings.text("기록 수정", "Edit entry")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                OutlinedTextField(
                    text,
                    onText,
                    enabled = !busy,
                    modifier = Modifier.testTag("edit-text"),
                )
                error?.let {
                    Text(strings.errorMessage(it), color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            Action(strings.text("저장", "Save"), "edit-save", enabled = !busy, onClick = onSave)
        },
        dismissButton = {
            Action(strings.text("닫기", "Close"), "edit-close", enabled = !busy, onClick = onClose)
        },
    )
}

@Composable
internal fun MigrateEntryDialog(
    date: String,
    page: String,
    busy: Boolean,
    error: String?,
    strings: JournalStrings,
    onDate: (String) -> Unit,
    onPage: (String) -> Unit,
    onSave: () -> Unit,
    onClose: () -> Unit,
) {
    AlertDialog(
        containerColor = MaterialTheme.colorScheme.surface,
        titleContentColor = MaterialTheme.colorScheme.onBackground,
        textContentColor = MaterialTheme.colorScheme.onSurface,
        shape = RoundedCornerShape(12.dp),
        tonalElevation = 0.dp,
        onDismissRequest = { if (!busy) onClose() },
        title = { Text(strings.text("할 일 이월", "Migrate task")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                FlowRow {
                    JournalPage.basic
                        .filter { it != JournalPage.Index }
                        .forEach { target ->
                            Action(
                                strings.pageLabel(target.key),
                                "migration-${target.key}",
                                page == target.key,
                            ) {
                                onPage(target.key)
                            }
                        }
                }
                OutlinedTextField(
                    date,
                    onDate,
                    label = { Text("YYYY-MM-DD") },
                    modifier = Modifier.testTag("migration-date"),
                )
                error?.let {
                    Text(strings.errorMessage(it), color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            Action(
                strings.text("이월하기", "Migrate"),
                "migration-commit",
                enabled = !busy,
                onClick = onSave,
            )
        },
        dismissButton = {
            Action(
                strings.text("닫기", "Close"),
                "migration-close",
                enabled = !busy,
                onClick = onClose,
            )
        },
    )
}
