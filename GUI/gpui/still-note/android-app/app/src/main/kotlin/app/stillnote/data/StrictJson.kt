package app.stillnote.data

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.jsonPrimitive

/** Reject repeated keys before JsonObject can collapse them into a valid-looking DTO. */
internal fun strictJson(value: String): JsonElement {
    DuplicateKeyScanner(value).check()
    return Json.parseToJsonElement(value)
}

private class DuplicateKeyScanner(private val source: String) {
    private var position = 0

    fun check() {
        value(0)
        whitespace()
        require(position == source.length)
    }

    private fun whitespace() {
        while (position < source.length && source[position] in " \t\r\n") position++
    }

    private fun value(depth: Int) {
        whitespace()
        require(position < source.length) { "Expected JSON value" }
        when (source[position]) {
            '{' -> {
                require(depth < 128) { "JSON nesting exceeds 128" }
                objectValue(depth + 1)
            }
            '[' -> {
                require(depth < 128) { "JSON nesting exceeds 128" }
                arrayValue(depth + 1)
            }
            '"' -> stringToken()
            else -> while (position < source.length && source[position] !in ",]} \t\r\n") position++
        }
    }

    private fun objectValue(depth: Int) {
        position++
        whitespace()
        if (source[position] == '}') {
            position++
            return
        }
        val keys = mutableSetOf<String>()
        while (true) {
            whitespace()
            val key = Json.parseToJsonElement(stringToken()).jsonPrimitive.content
            require(keys.add(key)) { "Duplicate JSON key: $key" }
            whitespace()
            require(source[position++] == ':')
            value(depth)
            whitespace()
            val delimiter = source[position++]
            if (delimiter == '}') return
            require(delimiter == ',')
        }
    }

    private fun arrayValue(depth: Int) {
        position++
        whitespace()
        if (source[position] == ']') {
            position++
            return
        }
        while (true) {
            value(depth)
            whitespace()
            val delimiter = source[position++]
            if (delimiter == ']') return
            require(delimiter == ',')
        }
    }

    private fun stringToken(): String {
        val start = position
        require(source[position++] == '"')
        while (position < source.length) {
            when (source[position++]) {
                '\\' -> position++
                '"' -> {
                    val token = source.substring(start, position)
                    val decoded = Json.parseToJsonElement(token).jsonPrimitive.content
                    var index = 0
                    while (index < decoded.length) {
                        val unit = decoded[index++]
                        if (unit.isHighSurrogate()) {
                            require(index < decoded.length && decoded[index].isLowSurrogate()) {
                                "Unpaired JSON surrogate"
                            }
                            index++
                        } else require(!unit.isLowSurrogate()) { "Unpaired JSON surrogate" }
                    }
                    return token
                }
            }
        }
        error("Unterminated JSON string")
    }
}
