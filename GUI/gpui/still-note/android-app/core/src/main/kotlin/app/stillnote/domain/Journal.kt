package app.stillnote.domain

import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.ResolverStyle
import java.util.Locale
import java.util.UUID

enum class Kind {
    Task,
    Event,
    Note,
}

enum class Status {
    Open,
    Complete,
    Cancelled,
    Migrated,
    Scheduled,
}

enum class Filter {
    All,
    Open,
    Complete,
}

sealed interface Log {
    data object Daily : Log

    data object Monthly : Log

    data object Future : Log

    data class Collection(val id: UUID) : Log
}

data class Entry(
    val id: UUID,
    val date: LocalDate,
    val log: Log,
    val kind: Kind,
    val status: Status = Status.Open,
    val important: Boolean = false,
    val text: String,
    val migratedFrom: UUID? = null,
    val migratedTo: UUID? = null,
) {
    fun symbol(): String =
        when (status) {
            Status.Complete -> "×"
            Status.Cancelled -> "⊘"
            Status.Migrated -> ">"
            Status.Scheduled -> "<"
            Status.Open ->
                when (kind) {
                    Kind.Task -> "•"
                    Kind.Event -> "○"
                    Kind.Note -> "–"
                }
        }

    fun isOpenTask() = kind == Kind.Task && status == Status.Open

    fun frozen() = status == Status.Migrated || status == Status.Scheduled
}

data class Collection(val id: UUID, val name: String)

class DomainException(val code: String) : IllegalArgumentException(code)

private fun checkRule(value: Boolean, code: String) {
    if (!value) throw DomainException(code)
}

fun parseDate(value: String): LocalDate {
    val date =
        try {
            LocalDate.parse(
                value,
                DateTimeFormatter.ofPattern("uuuu-MM-dd").withResolverStyle(ResolverStyle.STRICT),
            )
        } catch (_: Exception) {
            throw DomainException("date")
        }
    checkRule(date.year in 1..9999, "date")
    return date
}

fun shiftMonth(date: LocalDate, delta: Int): LocalDate =
    date.withDayOfMonth(1).plusMonths(delta.toLong()).also { checkRule(it.year in 1..9999, "date") }

fun sameLocation(a: LocalDate, al: Log, b: LocalDate, bl: Log): Boolean =
    al == bl &&
        when (al) {
            Log.Daily -> a == b
            Log.Monthly,
            Log.Future -> a.year == b.year && a.month == b.month
            is Log.Collection -> true
        }

data class Journal(
    val version: Int = 1,
    val entries: List<Entry> = emptyList(),
    val collections: List<Collection> = emptyList(),
) {
    fun entry(id: UUID): Entry =
        entries.find { it.id == id } ?: throw DomainException("entry_missing")

    private fun validateLog(log: Log) {
        checkRule(
            log !is Log.Collection || collections.any { it.id == log.id },
            "collection_missing",
        )
    }

    fun validate() {
        checkRule(version == 1, "version")
        val ids = mutableSetOf<UUID>()
        val names = mutableSetOf<String>()
        collections.forEach {
            checkRule(
                it.id != UUID(0, 0) &&
                    ids.add(it.id) &&
                    it.name.trim().isNotEmpty() &&
                    names.add(it.name.trim()),
                "collection_invalid",
            )
        }
        entries.forEach { e ->
            checkRule(e.id != UUID(0, 0) && ids.add(e.id), "id")
            checkRule(e.text.trim().isNotEmpty() && e.date.year in 1..9999, "entry_invalid")
            validateLog(e.log)
            checkRule(
                e.kind == Kind.Task || e.status in listOf(Status.Open, Status.Cancelled),
                "task_only",
            )
            checkRule(e.frozen() == (e.migratedTo != null), "migration_link")
            e.migratedTo?.let { id ->
                val t = entry(id)
                checkRule(
                    t.migratedFrom == e.id &&
                        t.id != e.id &&
                        t.kind == Kind.Task &&
                        t.log !is Log.Collection &&
                        e.kind == Kind.Task &&
                        !sameLocation(e.date, e.log, t.date, t.log),
                    "migration_link",
                )
                checkRule((e.status == Status.Scheduled) == (t.log == Log.Future), "migration_link")
            }
            e.migratedFrom?.let {
                val s = entry(it)
                checkRule(s.migratedTo == e.id && s.id != e.id, "migration_link")
            }
        }
        entries.forEach {
            val seen = mutableSetOf<UUID>()
            var e = it
            while (e.migratedTo != null) {
                checkRule(seen.add(e.id), "migration_cycle")
                e = entry(e.migratedTo!!)
            }
        }
    }

    fun addCollection(name: String, id: UUID): Journal {
        val n = name.trim()
        checkRule(n.isNotEmpty(), "collection_empty")
        checkRule(collections.none { it.name == n }, "collection_duplicate")
        return copy(collections = collections + Collection(id, n)).also { it.validate() }
    }

    fun addEntry(date: LocalDate, log: Log, kind: Kind, text: String, id: UUID): Journal {
        checkRule(text.trim().isNotEmpty(), "text_empty")
        validateLog(log)
        return copy(entries = entries + Entry(id, date, log, kind, text = text.trim())).also {
            it.validate()
        }
    }

    private fun update(id: UUID, operation: (Entry) -> Entry): Journal {
        val e = entry(id)
        checkRule(!e.frozen(), "frozen")
        return copy(entries = entries.map { if (it.id == id) operation(e) else it }).also {
            it.validate()
        }
    }

    fun setStatus(id: UUID, status: Status): Journal =
        update(id) {
            checkRule(
                status in listOf(Status.Open, Status.Complete, Status.Cancelled),
                "use_migration",
            )
            checkRule(it.kind == Kind.Task || status != Status.Complete, "task_only")
            it.copy(status = status)
        }

    fun editEntry(id: UUID, text: String): Journal =
        update(id) {
            checkRule(text.trim().isNotEmpty(), "text_empty")
            it.copy(text = text.trim())
        }

    fun toggleImportant(id: UUID): Journal = update(id) { it.copy(important = !it.important) }

    fun migrate(id: UUID, date: LocalDate, log: Log, targetId: UUID): Journal {
        val s = entry(id)
        checkRule(s.isOpenTask(), "open_task_only")
        validateLog(log)
        checkRule(log !is Log.Collection, "migration_collection")
        checkRule(!sameLocation(s.date, s.log, date, log), "same_location")
        val target =
            Entry(
                targetId,
                date,
                log,
                Kind.Task,
                important = s.important,
                text = s.text,
                migratedFrom = id,
            )
        return copy(
                entries =
                    entries.map {
                        if (it.id == id)
                            it.copy(
                                status =
                                    if (log == Log.Future) Status.Scheduled else Status.Migrated,
                                migratedTo = targetId,
                            )
                        else it
                    } + target
            )
            .also { it.validate() }
    }

    fun visible(date: LocalDate, log: Log): List<Entry> =
        entries.filter { sameLocation(it.date, it.log, date, log) }

    fun search(query: String, filter: Filter): List<Entry> {
        val q = query.trim().lowercase(Locale.ROOT)
        return entries.filter {
            it.text.lowercase(Locale.ROOT).contains(q) && matchesFilter(it, filter)
        }
    }
}

fun matchesFilter(entry: Entry, filter: Filter): Boolean =
    when (filter) {
        Filter.All -> true
        Filter.Open -> entry.status == Status.Open
        Filter.Complete -> entry.status == Status.Complete
    }
