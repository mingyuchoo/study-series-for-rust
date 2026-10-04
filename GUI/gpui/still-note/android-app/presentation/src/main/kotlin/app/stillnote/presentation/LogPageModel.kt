package app.stillnote.presentation

import app.stillnote.application.JournalQuery
import app.stillnote.application.indexLocations
import app.stillnote.domain.*
import java.time.LocalDate

/** Pure read model: Compose receives results rather than issuing its own queries. */
data class LogPageModel(
    val page: JournalPage,
    val date: LocalDate,
    val entries: List<Entry>,
    val indexLocations: List<Entry>,
    val calendar: List<CalendarDay>,
    val searching: Boolean,
    val openTaskCount: Int,
) {
    companion object {
        fun project(
            journal: Journal,
            page: JournalPage,
            date: LocalDate,
            search: String,
            filter: Filter,
        ): LogPageModel {
            val searching = search.isNotBlank()
            val entries =
                JournalQuery(date, page.log, search, filter, page == JournalPage.Index)
                    .entries(journal)
            return LogPageModel(
                page,
                date,
                entries,
                if (page == JournalPage.Index && !searching) journal.indexLocations()
                else emptyList(),
                if (page == JournalPage.Monthly && !searching) monthlyCalendar(journal, date)
                else emptyList(),
                searching,
                entries.count { it.isOpenTask() },
            )
        }
    }
}
