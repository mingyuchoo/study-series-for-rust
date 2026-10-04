package app.stillnote.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import app.stillnote.domain.Kind
import app.stillnote.presentation.JournalStrings

@Composable
internal fun EntryCapturePanel(
    draft: TextFieldValue,
    kind: Kind,
    busy: Boolean,
    canCapture: Boolean,
    error: String?,
    strings: JournalStrings,
    onDraft: (TextFieldValue) -> Unit,
    onKind: (Kind) -> Unit,
    onCapture: () -> Unit,
) {
    Panel(Modifier.testTag("composer")) {
        FlowRow {
            Kind.entries.forEach { option ->
                Action(
                    when (option) {
                        Kind.Task -> strings.text("• 할 일", "• Task")
                        Kind.Event -> strings.text("○ 이벤트", "○ Event")
                        Kind.Note -> strings.text("– 메모", "– Note")
                    },
                    "kind-$option",
                    kind == option,
                ) {
                    onKind(option)
                }
            }
        }
        OutlinedTextField(
            draft,
            onDraft,
            enabled = !busy,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions =
                KeyboardActions(onDone = { if (draft.composition == null) onCapture() }),
            label = { Text(strings.text("지금 떠오르는 생각을 기록하세요", "Write what's on your mind")) },
            isError = error == "text_empty",
            supportingText =
                if (error == "text_empty") {
                    { Text(strings.errorMessage(error)) }
                } else null,
            modifier = Modifier.fillMaxWidth().testTag("draft"),
            minLines = 2,
        )
        Button(
            onClick = onCapture,
            enabled = canCapture,
            shape = RoundedCornerShape(8.dp),
            modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).testTag("capture"),
        ) {
            Text(strings.text("기록하기", "Capture"))
        }
    }
}
