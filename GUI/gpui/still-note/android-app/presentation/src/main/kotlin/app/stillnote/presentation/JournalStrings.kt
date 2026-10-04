package app.stillnote.presentation

import app.stillnote.domain.*

/** Deterministic display strings, independent of Compose and Android resources. */
class JournalStrings(private val language: Language) {
    fun text(korean: String, english: String) = if (language == Language.Korean) korean else english

    fun pageLabel(name: String) = label(name, language == Language.Korean)

    fun location(entry: Entry, journal: Journal) =
        location(entry, journal, language == Language.Korean)

    fun errorMessage(code: String): String =
        when (code) {
            "text_empty" -> text("기록할 내용을 입력해 주세요", "Enter text to record")
            "collection_empty" -> text("컬렉션 이름을 입력해 주세요", "Enter a collection name")
            "collection_duplicate" -> text("이미 있는 컬렉션 이름입니다", "Collection name already exists")
            "date" ->
                text("날짜는 YYYY-MM-DD, 0001~9999년 범위여야 합니다", "Use YYYY-MM-DD within years 0001–9999")
            "same_location" -> text("다른 날짜 또는 로그를 선택해 주세요", "Choose a different date or log")
            "journal_load",
            "journal_protected" ->
                text(
                    "저널을 읽을 수 없습니다. 원본을 보존했습니다. 앱을 닫고 journal.json과 journal.json.bak를 백업한 뒤 정상 파일을 복원하고 재시작하세요.",
                    "Journal could not be loaded. Original preserved. Close the app, copy journal.json and journal.json.bak, restore a valid journal, then restart.",
                )
            "journal_conflict" ->
                text(
                    "다른 프로그램이 파일을 변경했습니다. 앱을 재시작해 주세요",
                    "Journal changed externally. Restart the app",
                )
            "settings_load",
            "settings_protected" ->
                text(
                    "설정 원본을 보존했습니다. settings.json을 확인한 뒤 재시작하세요. 선택은 현재 세션에 적용됩니다.",
                    "Settings original preserved. Check settings.json and restart. Selection applies to this session.",
                )
            "settings_conflict" ->
                text(
                    "설정이 외부에서 변경되었습니다. 선택은 현재 세션에 적용됩니다. 재시작하세요.",
                    "Settings changed externally. Selection applies to this session. Restart.",
                )
            "settings_save" ->
                text(
                    "설정을 저장하지 못했습니다. 선택은 현재 세션에 적용됩니다",
                    "Settings could not be saved. Selection applies to this session",
                )
            else ->
                text(
                    "변경을 저장하지 못했습니다. 입력과 원본을 보존했습니다 ($code)",
                    "Change could not be saved. Input and original preserved ($code)",
                )
        }
}

private fun label(name: String, ko: Boolean) =
    if (!ko) name
    else
        when (name) {
            "Daily" -> "일간 로그"
            "Monthly" -> "월간 로그"
            "Future" -> "미래 로그"
            "Index" -> "인덱스"
            else -> name
        }

private fun location(e: Entry, journal: Journal, ko: Boolean) =
    when (val l = e.log) {
        Log.Daily -> label("Daily", ko)
        Log.Monthly -> label("Monthly", ko)
        Log.Future -> label("Future", ko)
        is Log.Collection -> journal.collections.find { it.id == l.id }?.name ?: ""
    }
