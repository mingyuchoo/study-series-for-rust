package app.stillnote.data

import app.stillnote.domain.*
import kotlinx.serialization.json.*

object SettingsCodec {
    fun decode(value: String): Settings {
        val o = strictJson(value).jsonObject
        require(o.keys == setOf("language", "theme"))
        return Settings(
            when (o.getValue("language").jsonPrimitive.content) {
                "korean" -> Language.Korean
                "english" -> Language.English
                else -> error("language")
            },
            when (o.getValue("theme").jsonPrimitive.content) {
                "system" -> ThemeMode.System
                "light" -> ThemeMode.Light
                "dark" -> ThemeMode.Dark
                else -> error("theme")
            },
        )
    }

    fun encode(settings: Settings): String =
        "{\n  \"language\": \"${settings.language.name.lowercase()}\",\n  \"theme\": \"${settings.theme.name.lowercase()}\"\n}"
}
