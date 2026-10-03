package app.stillnote

import app.stillnote.application.JournalQuery
import app.stillnote.application.indexLocations
import app.stillnote.domain.*
import java.util.UUID
import org.junit.Assert.*
import org.junit.Test

class JournalQueryTest {
    private val date = parseDate("2024-02-29")
    private val collection = UUID(0, 20)
    private val journal =
        Journal()
            .addCollection("books", collection)
            .addEntry(date, Log.Daily, Kind.Task, "Read", UUID(0, 1))
            .addEntry(date.plusDays(1), Log.Daily, Kind.Note, "Read later", UUID(0, 2))
            .addEntry(date, Log.Monthly, Kind.Task, "Read monthly", UUID(0, 3))
            .setStatus(UUID(0, 3), Status.Complete)
            .addEntry(date, Log.Collection(collection), Kind.Event, "Reading", UUID(0, 4))

    @Test
    fun searchOverridesSelectedLocationAndIndexButKeepsStatusFilter() {
        val query = JournalQuery(date, Log.Future, " READ ", Filter.Open, index = true)
        assertEquals(
            listOf(UUID(0, 1), UUID(0, 2), UUID(0, 4)),
            query.entries(journal).map { it.id },
        )
        assertEquals(
            listOf(UUID(0, 3)),
            query.copy(filter = Filter.Complete).entries(journal).map { it.id },
        )
        assertEquals(4, journal.entries.size)
    }

    @Test
    fun blankSearchUsesLocationWhileIndexHasNoEntryRows() {
        val query = JournalQuery(date, Log.Daily, "  ")
        assertEquals(listOf(journal.entry(UUID(0, 1))), query.entries(journal))
        assertTrue(query.copy(index = true).entries(journal).isEmpty())
        assertTrue(query.copy(log = Log.Monthly, filter = Filter.Open).entries(journal).isEmpty())
        assertEquals(
            listOf(journal.entry(UUID(0, 4))),
            query.copy(log = Log.Collection(collection), date = date.plusYears(1)).entries(journal),
        )
    }

    @Test
    fun indexGroupsDailyByDayMonthlyAndFutureByMonthAndCollectionsByIdentity() {
        val expanded =
            journal
                .addEntry(date.minusDays(1), Log.Monthly, Kind.Note, "same month", UUID(0, 5))
                .addEntry(date, Log.Future, Kind.Note, "future", UUID(0, 6))
                .addEntry(date.minusDays(1), Log.Future, Kind.Note, "same future", UUID(0, 7))
                .addEntry(date.plusMonths(1), Log.Monthly, Kind.Note, "next month", UUID(0, 8))
                .addEntry(
                    date.plusYears(1),
                    Log.Collection(collection),
                    Kind.Note,
                    "same collection",
                    UUID(0, 9),
                )
        assertEquals(
            listOf(1L, 2L, 3L, 4L, 6L, 8L).map { UUID(0, it) },
            expanded.indexLocations().map { it.id },
        )
    }
}
