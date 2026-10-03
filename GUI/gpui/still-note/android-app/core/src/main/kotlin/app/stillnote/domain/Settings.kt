package app.stillnote.domain

enum class Language {
    Korean,
    English,
}

enum class ThemeMode {
    System,
    Light,
    Dark,
}

data class Settings(
    val language: Language = Language.Korean,
    val theme: ThemeMode = ThemeMode.System,
)
