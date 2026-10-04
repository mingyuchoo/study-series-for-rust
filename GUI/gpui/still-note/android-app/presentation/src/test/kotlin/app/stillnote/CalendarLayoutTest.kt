package app.stillnote

import app.stillnote.domain.Journal
import app.stillnote.presentation.*
import java.time.LocalDate
import org.junit.Assert.*
import org.junit.Test

class CalendarLayoutTest {
    private fun weeks(date: String) =
        calendarWeeks(monthlyCalendar(Journal(), LocalDate.parse(date)))

    @Test
    fun octoberDatesAlignWithThursdayAndSunday() {
        val weeks = weeks("2026-10-04")
        assertEquals(5, weeks.size)
        assertTrue(weeks.all { it.size == 7 })
        assertTrue(weeks.first().take(4).all { it == null })
        assertEquals(1, weeks[0][4]!!.date.dayOfMonth)
        assertEquals(4, weeks[1][0]!!.date.dayOfMonth)
        assertEquals(31, weeks.last()[6]!!.date.dayOfMonth)
    }

    @Test
    fun leapFebruaryHasTrailingBlanksAndEveryDateOnce() {
        val weeks = weeks("2024-02-29")
        assertEquals(29, weeks.last()[4]!!.date.dayOfMonth)
        assertTrue(weeks.last().drop(5).all { it == null })
        assertEquals((1..29).toList(), weeks.flatten().filterNotNull().map { it.date.dayOfMonth })
    }

    @Test
    fun sixWeekMonthAndEmptyCalendarAreSupported() {
        assertEquals(6, weeks("2026-08-01").size)
        assertEquals(1, weeks("2026-02-01").first()[0]!!.date.dayOfMonth)
        assertTrue(calendarWeeks(emptyList()).isEmpty())
    }
}
