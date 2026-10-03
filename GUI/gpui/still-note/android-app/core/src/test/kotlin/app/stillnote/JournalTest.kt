package app.stillnote

import app.stillnote.domain.*
import java.time.LocalDate
import java.util.UUID
import org.junit.Assert.*
import org.junit.Test

class JournalTest {
    private val date = parseDate("2024-02-29")

    private fun id(n: Long) = UUID(0, n)

    private fun rejected(action: () -> Unit) =
        assertThrows(IllegalArgumentException::class.java, action)

    @Test
    fun ac02_unicodeKindsWhitespaceAndSymbols() {
        var journal = Journal()
        listOf(Kind.Task, Kind.Event, Kind.Note).forEachIndexed { index, kind ->
            journal = journal.addEntry(date, Log.Daily, kind, "  한글 café 📚  ", id(index + 1L))
        }
        assertEquals(listOf("•", "○", "–"), journal.entries.map { it.symbol() })
        assertEquals(
            listOf("한글 café 📚", "한글 café 📚", "한글 café 📚"),
            journal.entries.map { it.text },
        )
        listOf("", " \t\n", "\u3000\u2003").forEach {
            rejected { journal.addEntry(date, Log.Daily, Kind.Task, it, id(99)) }
        }
        assertEquals(3, journal.entries.size)
        assertEquals(emptyList<Entry>(), Journal().entries)
        journal.validate()
    }

    @Test
    fun ac02_transitionsEditImportanceAndNonTaskRestrictions() {
        val initial = Journal().addEntry(date, Log.Daily, Kind.Task, "초안", id(1))
        val edited = initial.editEntry(id(1), "  수정 📝 ").toggleImportant(id(1))
        assertEquals("초안", initial.entry(id(1)).text)
        assertEquals("수정 📝", edited.entry(id(1)).text)
        assertTrue(edited.entry(id(1)).important)
        assertEquals("×", edited.setStatus(id(1), Status.Complete).entry(id(1)).symbol())
        assertEquals(
            "•",
            edited
                .setStatus(id(1), Status.Complete)
                .setStatus(id(1), Status.Open)
                .entry(id(1))
                .symbol(),
        )
        assertEquals("⊘", edited.setStatus(id(1), Status.Cancelled).entry(id(1)).symbol())
        rejected { edited.editEntry(id(1), " ") }
        rejected { edited.setStatus(id(1), Status.Migrated) }
        listOf(Kind.Note, Kind.Event).forEach { kind ->
            val j = Journal().addEntry(date, Log.Daily, kind, "note", id(1))
            rejected { j.setStatus(id(1), Status.Complete) }
            rejected { j.migrate(id(1), date.plusDays(1), Log.Daily, id(99)) }
            assertEquals(Status.Cancelled, j.setStatus(id(1), Status.Cancelled).entry(id(1)).status)
        }
    }

    @Test
    fun ac03_strictDateLeapYearAndMonthBounds() {
        assertEquals(29, date.dayOfMonth)
        listOf("2023-02-29", "2024-02-30", "2026-13-01", "0000-01-01", "10000-01-01", "garbage")
            .forEach { rejected { parseDate(it) } }
        assertEquals(parseDate("2027-01-01"), shiftMonth(parseDate("2026-12-31"), 1))
        assertEquals(parseDate("2024-02-01"), shiftMonth(parseDate("2024-03-31"), -1))
        rejected { shiftMonth(parseDate("0001-01-01"), -1) }
        rejected { shiftMonth(parseDate("9999-12-31"), 1) }
        rejected {
            Journal().addEntry(LocalDate.of(0, 1, 1), Log.Daily, Kind.Task, "invalid", id(99))
        }
    }

    @Test
    fun ac03_ac04_visibilityAndCollectionUniqueness() {
        val journal =
            Journal()
                .addCollection("  독서 📚 ", id(100))
                .addEntry(date, Log.Daily, Kind.Task, "daily", id(1))
                .addEntry(date, Log.Monthly, Kind.Event, "monthly", id(2))
                .addEntry(date, Log.Future, Kind.Note, "future", id(3))
                .addEntry(date, Log.Collection(id(100)), Kind.Note, "book", id(4))
        assertEquals("독서 📚", journal.collections.single().name)
        rejected { journal.addCollection("독서 📚", id(99)) }
        rejected { journal.addCollection("  ", id(99)) }
        rejected { journal.addEntry(date, Log.Collection(id(200)), Kind.Note, "missing", id(99)) }
        assertTrue(journal.visible(date.plusDays(1), Log.Daily).isEmpty())
        assertEquals(
            listOf(id(2)),
            journal.visible(parseDate("2024-02-01"), Log.Monthly).map { it.id },
        )
        assertTrue(journal.visible(parseDate("2025-02-01"), Log.Monthly).isEmpty())
        assertEquals(
            listOf(id(3)),
            journal.visible(parseDate("2024-02-01"), Log.Future).map { it.id },
        )
        assertEquals(
            listOf(id(4)),
            journal.visible(parseDate("9999-12-31"), Log.Collection(id(100))).map { it.id },
        )
    }

    @Test
    fun ac05_globalSearchLowercaseUnicodeAndOpenNotesEvents() {
        val journal =
            Journal()
                .addCollection("books", id(100))
                .addEntry(date, Log.Daily, Kind.Task, "책 REVIEW", id(1))
                .addEntry(date, Log.Future, Kind.Task, "책 café", id(2))
                .setStatus(id(2), Status.Complete)
                .addEntry(date, Log.Collection(id(100)), Kind.Note, "책 review", id(3))
                .addEntry(date, Log.Monthly, Kind.Event, "책 event", id(4))
        assertEquals(
            listOf(id(1), id(2), id(3), id(4)),
            journal.search(" 책 ", Filter.All).map { it.id },
        )
        assertEquals(listOf(id(1), id(3), id(4)), journal.search("책", Filter.Open).map { it.id })
        assertEquals(listOf(id(2)), journal.search("책", Filter.Complete).map { it.id })
        assertEquals(listOf(id(1), id(3)), journal.search("ReViEw", Filter.All).map { it.id })
        assertTrue(journal.search("missing", Filter.All).isEmpty())
    }

    @Test
    fun ac06_migrationCopiesImportanceFreezesSourceAndHasReciprocalLinks() {
        listOf(Log.Daily, Log.Monthly, Log.Future).forEach { log ->
            val initial =
                Journal().addEntry(date, Log.Daily, Kind.Task, "읽기", id(1)).toggleImportant(id(1))
            val migrated = initial.migrate(id(1), parseDate("2025-01-01"), log, id(2))
            val source = migrated.entry(id(1))
            val target = migrated.entry(id(2))
            assertEquals(date, source.date)
            assertEquals(Log.Daily, source.log)
            assertEquals(id(2), source.migratedTo)
            assertEquals(if (log == Log.Future) "<" else ">", source.symbol())
            assertEquals(id(1), target.migratedFrom)
            assertEquals(Status.Open, target.status)
            assertEquals(log, target.log)
            assertTrue(target.important)
            assertEquals(source.text, target.text)
            rejected { migrated.editEntry(id(1), "change") }
            rejected { migrated.toggleImportant(id(1)) }
            rejected { migrated.setStatus(id(1), Status.Open) }
            rejected { migrated.migrate(id(1), date.plusDays(1), Log.Daily, id(99)) }
            migrated.validate()
        }
    }

    @Test
    fun ac06_sameLocationsClosedTasksAndCollectionTargetsRejected() {
        listOf(Log.Daily, Log.Monthly, Log.Future).forEach { log ->
            val j = Journal().addEntry(date, log, Kind.Task, "task", id(1))
            rejected {
                j.migrate(
                    id(1),
                    if (log == Log.Daily) date else date.withDayOfMonth(1),
                    log,
                    id(99),
                )
            }
            rejected {
                j.setStatus(id(1), Status.Complete)
                    .migrate(id(1), date.plusMonths(1), Log.Daily, id(99))
            }
        }
        val j =
            Journal()
                .addCollection("projects", id(100))
                .addEntry(date, Log.Daily, Kind.Task, "task", id(1))
        rejected { j.migrate(id(1), date.plusDays(1), Log.Collection(id(100)), id(99)) }
        j.migrate(id(1), date, Log.Monthly, id(99)).validate()
    }

    @Test
    fun ac06_ac08_validationRejectsMissingLinksCyclesIdsAndKinds() {
        val j =
            Journal()
                .addEntry(date, Log.Daily, Kind.Task, "task", id(1))
                .migrate(id(1), date.plusDays(1), Log.Daily, id(2))
        rejected { j.copy(entries = listOf(j.entry(id(1)))).validate() }
        rejected {
            j.copy(
                    entries =
                        j.entries.map { if (it.id == id(2)) it.copy(migratedFrom = null) else it }
                )
                .validate()
        }
        val cycle =
            j.copy(
                entries =
                    listOf(
                        j.entry(id(1)).copy(migratedFrom = id(2)),
                        j.entry(id(2)).copy(status = Status.Migrated, migratedTo = id(1)),
                    )
            )
        rejected { cycle.validate() }
        rejected { j.copy(version = 2).validate() }
        rejected {
            Journal(entries = listOf(Entry(id(0), date, Log.Daily, Kind.Note, text = "note")))
                .validate()
        }
        rejected {
            Journal(
                    entries =
                        listOf(
                            Entry(id(1), date, Log.Daily, Kind.Note, Status.Complete, text = "note")
                        )
                )
                .validate()
        }
        rejected { Journal(entries = listOf(j.entry(id(2)), j.entry(id(2)))).validate() }
        rejected {
            Journal(collections = listOf(Collection(id(1), "name"), Collection(id(2), " name ")))
                .validate()
        }
    }
}
