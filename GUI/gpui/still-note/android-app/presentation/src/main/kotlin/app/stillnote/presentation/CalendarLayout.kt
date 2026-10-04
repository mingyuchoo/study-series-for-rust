package app.stillnote.presentation

/** Sunday-first weeks; empty cells keep each date under its weekday heading. */
fun calendarWeeks(days: List<CalendarDay>): List<List<CalendarDay?>> {
    if (days.isEmpty()) return emptyList()
    val cells = List<CalendarDay?>(days.first().date.dayOfWeek.value % 7) { null } + days
    val trailing = (7 - cells.size % 7) % 7
    return (cells + List<CalendarDay?>(trailing) { null }).chunked(7)
}
