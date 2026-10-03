package app.stillnote.application

import app.stillnote.domain.*
import java.time.LocalDate
import java.time.YearMonth

/** Pure read use case. Search is global; an index without a query has no entry rows. */
data class JournalQuery(
    val date: LocalDate,
    val log: Log,
    val search: String = "",
    val filter: Filter = Filter.All,
    val index: Boolean = false,
) {
    fun entries(journal: Journal): List<Entry> =
        when {
            search.isNotBlank() -> journal.search(search, filter)
            index -> emptyList()
            else -> journal.visible(date, log).filter { matchesFilter(it, filter) }
        }
}

/** One representative per location, retaining journal insertion order. */
fun Journal.indexLocations(): List<Entry> =
    entries.distinctBy {
        when (val log = it.log) {
            Log.Daily -> log to it.date
            Log.Monthly,
            Log.Future -> log to YearMonth.from(it.date)
            is Log.Collection -> log to null
        }
    }
