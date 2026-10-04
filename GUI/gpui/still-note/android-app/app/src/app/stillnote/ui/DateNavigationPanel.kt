package app.stillnote.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import app.stillnote.presentation.JournalStrings

@Composable
internal fun DateNavigationPanel(
    dateText: String,
    error: String?,
    strings: JournalStrings,
    onDate: (String) -> Unit,
    onShift: (Int) -> Unit,
    onToday: () -> Unit,
) {
    Column {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Action("‹", "previous") { onShift(-1) }
            Action(strings.text("오늘", "Today"), "today", onClick = onToday)
            Action("›", "next") { onShift(1) }
        }
        OutlinedTextField(
            dateText,
            onDate,
            label = { Text(strings.text("날짜 YYYY-MM-DD", "Date YYYY-MM-DD")) },
            singleLine = true,
            isError = error == "date",
            supportingText =
                if (error == "date") {
                    { Text(strings.errorMessage(error)) }
                } else null,
            modifier = Modifier.fillMaxWidth().testTag("date"),
        )
    }
}
