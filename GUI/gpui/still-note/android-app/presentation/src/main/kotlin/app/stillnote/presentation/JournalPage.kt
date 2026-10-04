package app.stillnote.presentation

import app.stillnote.domain.*
import java.time.DateTimeException
import java.time.LocalDate
import java.time.YearMonth
import java.util.UUID

/** Typed destinations retain the existing persisted page keys. */
sealed interface JournalPage {
    val key: String
    val log: Log

    data object Daily : JournalPage {
        override val key = "Daily"
        override val log = Log.Daily
    }

    data object Monthly : JournalPage {
        override val key = "Monthly"
        override val log = Log.Monthly
    }

    data object Future : JournalPage {
        override val key = "Future"
        override val log = Log.Future
    }

    data object Index : JournalPage {
        override val key = "Index"
        override val log = Log.Daily
    }

    data class Collection(val id: UUID) : JournalPage {
        override val key = id.toString()
        override val log = Log.Collection(id)
    }

    companion object {
        val basic = listOf(Daily, Monthly, Future, Index)

        fun fromKey(key: String): JournalPage =
            basic.find { it.key == key }
                ?: try {
                    Collection(UUID.fromString(key))
                } catch (_: IllegalArgumentException) {
                    Daily
                }

        fun forLog(log: Log): JournalPage =
            when (log) {
                Log.Daily -> Daily
                Log.Monthly -> Monthly
                Log.Future -> Future
                is Log.Collection -> Collection(log.id)
            }
    }
}

fun JournalPage.shiftDate(date: LocalDate, delta: Int): LocalDate =
    try {
        when (this) {
            JournalPage.Monthly,
            JournalPage.Future -> shiftMonth(date, delta)
            else -> date.plusDays(delta.toLong()).also { parseDate(it.toString()) }
        }
    } catch (_: DateTimeException) {
        throw DomainException("date")
    }

data class CalendarDay(val date: LocalDate, val entryCount: Int)

fun monthlyCalendar(journal: Journal, date: LocalDate): List<CalendarDay> {
    val month = YearMonth.from(date)
    val counts =
        journal.entries
            .filter { it.log == Log.Daily && YearMonth.from(it.date) == month }
            .groupingBy { it.date }
            .eachCount()
    return (1..month.lengthOfMonth()).map { day ->
        val selected = month.atDay(day)
        CalendarDay(selected, counts[selected] ?: 0)
    }
}
