package app.stillnote.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import app.stillnote.domain.Filter
import app.stillnote.presentation.JournalStrings

@Composable
internal fun JournalSearchPanel(
    search: String,
    filter: Filter,
    strings: JournalStrings,
    onSearch: (String) -> Unit,
    onFilter: (Filter) -> Unit,
) {
    Column {
        OutlinedTextField(
            search,
            onSearch,
            label = { Text(strings.text("전체 기록 검색", "Search all entries")) },
            modifier = Modifier.fillMaxWidth().testTag("search"),
        )
        Spacer(Modifier.height(12.dp))
        FlowRow {
            Filter.entries.forEach { option ->
                Action(
                    when (option) {
                        Filter.All -> strings.text("모든 기록", "All")
                        Filter.Open -> strings.text("미완료", "Open")
                        Filter.Complete -> strings.text("완료", "Complete")
                    },
                    "filter-$option",
                    filter == option,
                ) {
                    onFilter(option)
                }
            }
            if (search.isNotEmpty())
                Action(strings.text("검색 지우기", "Clear search"), "search-clear") { onSearch("") }
        }
        if (search.isNotBlank())
            Text(
                strings.text("검색 결과 · 전체 로그", "Search results · All logs"),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(top = 12.dp),
            )
    }
}
