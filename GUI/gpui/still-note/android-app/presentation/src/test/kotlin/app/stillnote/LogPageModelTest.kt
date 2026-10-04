package app.stillnote

import app.stillnote.domain.*
import app.stillnote.presentation.*
import java.util.UUID
import org.junit.Assert.*
import org.junit.Test

class LogPageModelTest {
    private val date = parseDate("2024-02-29")
    private val collectionId = UUID(0, 10)
    private val journal =
        Journal()
            .addCollection("Reading", collectionId)
            .addEntry(date, Log.Daily, Kind.Task, "Read", UUID(0, 1))
            .addEntry(date, Log.Daily, Kind.Note, "Read notes", UUID(0, 2))
            .addEntry(date.plusDays(1), Log.Future, Kind.Task, "Read later", UUID(0, 3))
            .setStatus(UUID(0, 1), Status.Complete)

    @Test
    fun globalSearchHasNoCalendarOrIndexLinksAndCountsOnlyOpenTasks() {
        for (page in listOf(JournalPage.Monthly, JournalPage.Index)) {
            val model = LogPageModel.project(journal, page, date, " READ ", Filter.Open)
            assertEquals(listOf(UUID(0, 2), UUID(0, 3)), model.entries.map { it.id })
            assertTrue(model.searching)
            assertTrue(model.calendar.isEmpty())
            assertTrue(model.indexLocations.isEmpty())
            assertEquals(1, model.openTaskCount)
        }
        assertEquals(3, journal.entries.size)
    }

    @Test
    fun monthlyOverviewCountsDailyEntriesIncludingCompletedOnLeapDay() {
        val model = LogPageModel.project(journal, JournalPage.Monthly, date, "  ", Filter.Open)
        assertEquals(29, model.calendar.size)
        assertEquals(CalendarDay(date, 2), model.calendar.last())
        assertEquals(0, model.calendar.first().entryCount)
        assertTrue(model.entries.isEmpty())
    }

    @Test
    fun indexKeepsLocationNavigationWithoutRenderingEntryRows() {
        val model = LogPageModel.project(journal, JournalPage.Index, date, "", Filter.All)
        assertEquals(listOf(UUID(0, 1), UUID(0, 3)), model.indexLocations.map { it.id })
        assertTrue(model.entries.isEmpty())
        assertFalse(model.searching)
    }

    @Test
    fun persistedNavigationKeysRoundTripIncludingCollectionsAndFallback() {
        val pages = JournalPage.basic + JournalPage.Collection(collectionId)
        pages.forEach { assertEquals(it, JournalPage.fromKey(it.key)) }
        assertEquals(JournalPage.Daily, JournalPage.fromKey("not-a-page"))
        assertEquals(
            JournalPage.Collection(collectionId),
            JournalPage.forLog(Log.Collection(collectionId)),
        )
        assertEquals(parseDate("2024-03-01"), JournalPage.Monthly.shiftDate(date, 1))
        assertEquals(parseDate("2024-02-01"), JournalPage.Future.shiftDate(date, 0))
        assertEquals(date.plusDays(1), JournalPage.Daily.shiftDate(date, 1))
    }

    @Test
    fun dateNavigationRejectsValuesOutsideThePersistedYearRange() {
        for ((page, selected, delta) in
            listOf(
                Triple(JournalPage.Daily, parseDate("9999-12-31"), 1),
                Triple(JournalPage.Monthly, parseDate("0001-01-01"), -1),
            )) {
            try {
                page.shiftDate(selected, delta)
                fail("Out-of-range date accepted")
            } catch (error: DomainException) {
                assertEquals("date", error.code)
            }
        }
    }

    @Test
    fun localizationPreservesCollectionNamesAndActionableErrors() {
        val korean = JournalStrings(Language.Korean)
        val english = JournalStrings(Language.English)
        assertEquals("일간 로그", korean.pageLabel("Daily"))
        assertEquals("Daily", english.pageLabel("Daily"))
        val entry = Entry(UUID(0, 5), date, Log.Collection(collectionId), Kind.Note, text = "Book")
        assertEquals("Reading", korean.location(entry, journal))
        assertEquals("Reading", english.location(entry, journal))
        assertTrue(korean.errorMessage("journal_conflict").contains("재시작"))
        assertTrue(english.errorMessage("date").contains("YYYY-MM-DD"))
    }
}
