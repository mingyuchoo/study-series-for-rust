package app.stillnote.ui

import androidx.compose.foundation.layout.FlowRow
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import app.stillnote.domain.*
import app.stillnote.presentation.EntryIntent
import app.stillnote.presentation.JournalStrings

/** Emits intent; its caller owns editing, navigation, and command execution. */
@Composable
internal fun EntryCard(
    entry: Entry,
    location: String,
    searching: Boolean,
    busy: Boolean,
    strings: JournalStrings,
    onIntent: (EntryIntent) -> Unit,
) {
    val e = entry
    Panel(Modifier.testTag("entry-${e.id}")) {
        Text(
            "${if (e.important) "★ " else ""}${e.symbol()}  ${e.text}",
            style = MaterialTheme.typography.bodyLarge,
            color =
                if (e.important) MaterialTheme.colorScheme.primary
                else MaterialTheme.colorScheme.onSurface,
        )
        Text("${e.date} · $location", style = MaterialTheme.typography.bodySmall)
        FlowRow {
            if (searching)
                Action(strings.text("위치로 이동", "Open location"), "jump-${e.id}") {
                    onIntent(EntryIntent.Open(e.id))
                }
            if (!e.frozen()) {
                Action(strings.text("수정", "Edit"), "edit-${e.id}", enabled = !busy) {
                    onIntent(EntryIntent.Edit(e.id))
                }
                Action(strings.text("중요", "Important"), "important-${e.id}", e.important, !busy) {
                    onIntent(EntryIntent.ToggleImportant(e.id))
                }
                if (e.kind == Kind.Task)
                    Action(
                        if (e.status == Status.Complete) strings.text("재개", "Reopen")
                        else strings.text("완료", "Complete"),
                        "complete-${e.id}",
                        enabled = !busy,
                    ) {
                        onIntent(
                            EntryIntent.SetStatus(
                                e.id,
                                if (e.status == Status.Complete) Status.Open else Status.Complete,
                            )
                        )
                    }
                Action(
                    if (e.status == Status.Cancelled) strings.text("재개", "Reopen")
                    else strings.text("취소", "Cancel"),
                    "cancel-${e.id}",
                    enabled = !busy,
                ) {
                    onIntent(
                        EntryIntent.SetStatus(
                            e.id,
                            if (e.status == Status.Cancelled) Status.Open else Status.Cancelled,
                        )
                    )
                }
                if (e.isOpenTask())
                    Action(strings.text("이월", "Migrate"), "migrate-${e.id}", enabled = !busy) {
                        onIntent(EntryIntent.Migrate(e.id))
                    }
            }
            e.migratedFrom?.let { id ->
                Action(strings.text("← 원본", "← Source"), "source-${e.id}") {
                    onIntent(EntryIntent.Open(id))
                }
            }
            e.migratedTo?.let { id ->
                Action(strings.text("대상 →", "Target →"), "target-${e.id}") {
                    onIntent(EntryIntent.Open(id))
                }
            }
        }
    }
}
