package app.stillnote.data

import app.stillnote.domain.*
import java.util.UUID
import kotlinx.serialization.json.*

private fun strictUuid(value: String): UUID {
    require(
        Regex("[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
            .matches(value)
    )
    return UUID.fromString(value)
}

object JournalCodec {
    private val json = Json { prettyPrint = true }

    fun encode(journal: Journal): String =
        json.encodeToString(
            JsonObject.serializer(),
            buildJsonObject {
                put("version", journal.version)
                put(
                    "entries",
                    buildJsonArray {
                        journal.entries.forEach { e ->
                            add(
                                buildJsonObject {
                                    put("id", e.id.toString())
                                    put("date", e.date.toString())
                                    put(
                                        "log",
                                        when (val l = e.log) {
                                            Log.Daily -> JsonPrimitive("Daily")
                                            Log.Monthly -> JsonPrimitive("Monthly")
                                            Log.Future -> JsonPrimitive("Future")
                                            is Log.Collection ->
                                                buildJsonObject {
                                                    put("Collection", l.id.toString())
                                                }
                                        },
                                    )
                                    put("kind", e.kind.name)
                                    put("status", e.status.name)
                                    put("important", e.important)
                                    put("text", e.text)
                                    put(
                                        "migrated_from",
                                        e.migratedFrom?.let { JsonPrimitive(it.toString()) }
                                            ?: JsonNull,
                                    )
                                    put(
                                        "migrated_to",
                                        e.migratedTo?.let { JsonPrimitive(it.toString()) }
                                            ?: JsonNull,
                                    )
                                }
                            )
                        }
                    },
                )
                put(
                    "collections",
                    buildJsonArray {
                        journal.collections.forEach {
                            add(
                                buildJsonObject {
                                    put("id", it.id.toString())
                                    put("name", it.name)
                                }
                            )
                        }
                    },
                )
            },
        )

    fun decode(value: String): Journal {
        val o = strictJson(value).jsonObject
        fun JsonObject.str(key: String): String {
            val p = getValue(key).jsonPrimitive
            require(p.isString)
            return p.content
        }
        fun JsonObject.link(key: String) =
            get(key)?.let {
                if (it == JsonNull) null
                else strictUuid(it.jsonPrimitive.also { p -> require(p.isString) }.content)
            }
        return Journal(
                o.getValue("version").jsonPrimitive.also { require(!it.isString) }.int,
                o.getValue("entries").jsonArray.map { item ->
                    val e = item.jsonObject
                    val l = e.getValue("log")
                    val log =
                        if (l is JsonObject) {
                            require(l.keys == setOf("Collection"))
                            Log.Collection(strictUuid(l.str("Collection")))
                        } else
                            when (l.jsonPrimitive.content) {
                                "Daily" -> Log.Daily
                                "Monthly" -> Log.Monthly
                                "Future" -> Log.Future
                                else -> error("log")
                            }
                    Entry(
                        strictUuid(e.str("id")),
                        parseDate(e.str("date")),
                        log,
                        Kind.valueOf(e.str("kind")),
                        Status.valueOf(e.str("status")),
                        e.getValue("important")
                            .jsonPrimitive
                            .also { require(!it.isString) }
                            .boolean,
                        e.str("text"),
                        e.link("migrated_from"),
                        e.link("migrated_to"),
                    )
                },
                o.getValue("collections").jsonArray.map {
                    val c = it.jsonObject
                    Collection(strictUuid(c.str("id")), c.str("name"))
                },
            )
            .also { it.validate() }
    }
}
