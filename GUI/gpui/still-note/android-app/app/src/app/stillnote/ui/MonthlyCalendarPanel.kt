package app.stillnote.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.*
import androidx.compose.ui.unit.dp
import app.stillnote.domain.Language
import app.stillnote.presentation.CalendarDay
import app.stillnote.presentation.JournalStrings
import app.stillnote.presentation.calendarWeeks
import java.time.LocalDate

@Composable
internal fun MonthlyCalendarPanel(
    days: List<CalendarDay>,
    selectedDate: LocalDate,
    language: Language,
    strings: JournalStrings,
    onDay: (LocalDate) -> Unit,
) {
    Panel(Modifier.testTag("monthly-calendar")) {
        Text(strings.text("월간 달력", "Monthly calendar"), style = MaterialTheme.typography.titleLarge)
        Text(
            strings.text(
                "${selectedDate.year}년 ${selectedDate.monthValue}월",
                "${selectedDate.month} ${selectedDate.year}",
            ),
            style = MaterialTheme.typography.titleMedium,
        )
        val weekdays =
            if (language == Language.Korean) listOf("일", "월", "화", "수", "목", "금", "토")
            else listOf("Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat")
        Row(Modifier.fillMaxWidth()) {
            weekdays.forEachIndexed { index, label ->
                Box(Modifier.weight(1f).height(32.dp), contentAlignment = Alignment.Center) {
                    Text(
                        label,
                        color =
                            if (index == 0) MaterialTheme.colorScheme.error
                            else MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.labelMedium,
                    )
                }
            }
        }
        HorizontalDivider()
        calendarWeeks(days).forEach { week ->
            Row(Modifier.fillMaxWidth()) {
                week.forEachIndexed { index, day ->
                    if (day == null) Spacer(Modifier.weight(1f).height(64.dp))
                    else {
                        val isSelected = day.date == selectedDate
                        TextButton(
                            onClick = { onDay(day.date) },
                            modifier =
                                Modifier.weight(1f)
                                    .height(64.dp)
                                    .testTag("day-${day.date.dayOfMonth}")
                                    .semantics {
                                        selected = isSelected
                                        contentDescription =
                                            strings.text(
                                                "${day.date}, 기록 ${day.entryCount}개",
                                                "${day.date}, ${day.entryCount} entries",
                                            )
                                    },
                            contentPadding = PaddingValues(0.dp),
                            shape = RoundedCornerShape(8.dp),
                            colors =
                                ButtonDefaults.textButtonColors(
                                    containerColor =
                                        if (isSelected) MaterialTheme.colorScheme.primaryContainer
                                        else androidx.compose.ui.graphics.Color.Transparent,
                                    contentColor =
                                        if (isSelected) MaterialTheme.colorScheme.onPrimaryContainer
                                        else if (index == 0) MaterialTheme.colorScheme.error
                                        else MaterialTheme.colorScheme.onSurface,
                                ),
                        ) {
                            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                                Text(
                                    day.date.dayOfMonth.toString(),
                                    style = MaterialTheme.typography.bodyLarge,
                                )
                                if (day.entryCount > 0)
                                    Text(
                                        "· ${day.entryCount}",
                                        style = MaterialTheme.typography.labelSmall,
                                    )
                                else Spacer(Modifier.height(16.dp))
                            }
                        }
                    }
                }
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        }
    }
}
