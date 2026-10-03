package app.stillnote.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.stillnote.application.JournalCommand
import app.stillnote.domain.*
import app.stillnote.presentation.JournalViewModel
import java.time.LocalDate
import java.time.YearMonth
import java.util.UUID

@Composable
private fun saved(vm: JournalViewModel, key: String, default: String = ""): MutableState<String> {
    val state = rememberSaveable(key) { mutableStateOf(vm.recalled(key, default)) }
    LaunchedEffect(state.value) { vm.remember(key, state.value) }
    return state
}

@Composable
fun StillnoteApp(vm: JournalViewModel) {
    val state by vm.state.collectAsStateWithLifecycle()
    val ko = state.settings.language == Language.Korean
    fun tr(k: String, e: String) = if (ko) k else e
    var logName by saved(vm, "log", "Daily")
    var dateText by saved(vm, "date", LocalDate.now().toString())
    var search by saved(vm, "search")
    var filterName by saved(vm, "filter", "All")
    var kindName by saved(vm, "kind", "Task")
    var menu by rememberSaveable { mutableStateOf(false) }
    var collectionName by saved(vm, "collection-name")
    var editId by saved(vm, "edit-id")
    var editText by saved(vm, "edit-text")
    var migrationId by saved(vm, "migration-id")
    var migrationDate by saved(vm, "migration-date", LocalDate.now().plusDays(1).toString())
    var migrationLog by saved(vm, "migration-log", "Daily")
    var draft by
        rememberSaveable(stateSaver = TextFieldValue.Saver) {
            mutableStateOf(
                TextFieldValue(
                    vm.recalled("draft"),
                    TextRange(
                        vm.recalled("draft-cursor", "0").toIntOrNull() ?: 0,
                        vm.recalled("draft-end", "0").toIntOrNull() ?: 0,
                    ),
                )
            )
        }
    LaunchedEffect(draft) {
        vm.remember("draft", draft.text)
        vm.remember("draft-cursor", draft.selection.start.toString())
        vm.remember("draft-end", draft.selection.end.toString())
    }
    var selectedDate by saved(vm, "selected-date", LocalDate.now().toString())
    val date =
        try {
            parseDate(dateText)
        } catch (_: Exception) {
            parseDate(selectedDate)
        }
    LaunchedEffect(dateText) {
        try {
            selectedDate = parseDate(dateText).toString()
        } catch (_: Exception) {}
    }
    val log: Log =
        when (logName) {
            "Monthly" -> Log.Monthly
            "Future" -> Log.Future
            "Daily",
            "Index" -> Log.Daily
            else ->
                try {
                    Log.Collection(UUID.fromString(logName))
                } catch (_: Exception) {
                    Log.Daily
                }
        }
    val filter = Filter.valueOf(filterName)
    fun navigate(name: String) {
        logName = name
        search = ""
        editId = ""
        menu = false
    }
    fun jump(e: Entry) {
        dateText = e.date.toString()
        logName =
            when (val l = e.log) {
                Log.Daily -> "Daily"
                Log.Monthly -> "Monthly"
                Log.Future -> "Future"
                is Log.Collection -> l.id.toString()
            }
        search = ""
        editId = ""
    }
    fun capture() {
        try {
            val d = parseDate(dateText)
            val text = draft.text
            val kind = Kind.valueOf(kindName)
            vm.execute(JournalCommand.AddEntry(d, log, kind, text)) { draft = TextFieldValue() }
        } catch (_: Exception) {
            vm.report("date")
        }
    }
    fun createCollection() {
        val submitted = collectionName
        vm.execute(JournalCommand.AddCollection(submitted)) {
            collectionName = ""
            navigate(vm.state.value.journal.collections.last().id.toString())
        }
    }
    fun errorMessage(code: String): String =
        when (code) {
            "text_empty" -> tr("기록할 내용을 입력해 주세요", "Enter text to record")
            "collection_empty" -> tr("컬렉션 이름을 입력해 주세요", "Enter a collection name")
            "collection_duplicate" -> tr("이미 있는 컬렉션 이름입니다", "Collection name already exists")
            "date" ->
                tr("날짜는 YYYY-MM-DD, 0001~9999년 범위여야 합니다", "Use YYYY-MM-DD within years 0001–9999")
            "same_location" -> tr("다른 날짜 또는 로그를 선택해 주세요", "Choose a different date or log")
            "journal_load",
            "journal_protected" ->
                tr(
                    "저널을 읽을 수 없습니다. 원본을 보존했습니다. 앱을 닫고 journal.json과 journal.json.bak를 백업한 뒤 정상 파일을 복원하고 재시작하세요.",
                    "Journal could not be loaded. Original preserved. Close the app, copy journal.json and journal.json.bak, restore a valid journal, then restart.",
                )
            "journal_conflict" ->
                tr(
                    "다른 프로그램이 파일을 변경했습니다. 앱을 재시작해 주세요",
                    "Journal changed externally. Restart the app",
                )
            "settings_load",
            "settings_protected" ->
                tr(
                    "설정 원본을 보존했습니다. settings.json을 확인한 뒤 재시작하세요. 선택은 현재 세션에 적용됩니다.",
                    "Settings original preserved. Check settings.json and restart. Selection applies to this session.",
                )
            "settings_conflict" ->
                tr(
                    "설정이 외부에서 변경되었습니다. 선택은 현재 세션에 적용됩니다. 재시작하세요.",
                    "Settings changed externally. Selection applies to this session. Restart.",
                )
            "settings_save" ->
                tr(
                    "설정을 저장하지 못했습니다. 선택은 현재 세션에 적용됩니다",
                    "Settings could not be saved. Selection applies to this session",
                )
            else ->
                tr(
                    "변경을 저장하지 못했습니다. 입력과 원본을 보존했습니다 ($code)",
                    "Change could not be saved. Input and original preserved ($code)",
                )
        }
    BackHandler(menu || editId.isNotEmpty() || migrationId.isNotEmpty() || search.isNotEmpty()) {
        when {
            menu -> menu = false
            migrationId.isNotEmpty() -> migrationId = ""
            editId.isNotEmpty() -> editId = ""
            else -> search = ""
        }
    }
    StillnoteTheme(state.settings.theme) {
        Surface(color = MaterialTheme.colorScheme.background, modifier = Modifier.fillMaxSize()) {
            BoxWithConstraints(Modifier.safeDrawingPadding().imePadding()) {
                val expanded = maxWidth >= 840.dp
                Row(Modifier.fillMaxSize()) {
                    if (expanded)
                        Column(
                            Modifier.width(220.dp)
                                .fillMaxHeight()
                                .verticalScroll(rememberScrollState())
                                .padding(16.dp)
                        ) {
                            Text("Stillnote", style = MaterialTheme.typography.headlineMedium)
                            Spacer(Modifier.height(24.dp))
                            listOf("Daily", "Monthly", "Future", "Index").forEach { name ->
                                Action(label(name, ko), "nav-$name", name == logName) {
                                    navigate(name)
                                }
                            }
                            Text(
                                tr("컬렉션", "Collections"),
                                Modifier.padding(top = 16.dp),
                                fontWeight = FontWeight.Bold,
                            )
                            state.journal.collections.forEach { c ->
                                Action(c.name, "collection-${c.id}") { navigate(c.id.toString()) }
                            }
                        }
                    LazyColumn(
                        Modifier.weight(1f)
                            .fillMaxHeight()
                            .padding(horizontal = if (expanded) 28.dp else 16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                        contentPadding = PaddingValues(bottom = 24.dp),
                    ) {
                        item {
                            Row(
                                Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                Text(
                                    "Stillnote",
                                    style = MaterialTheme.typography.headlineMedium,
                                    modifier = Modifier.padding(vertical = 12.dp),
                                )
                                Action(tr("메뉴 · 설정", "Menu · Settings"), "menu") { menu = !menu }
                            }
                            if (!expanded)
                                FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                                    listOf("Daily", "Monthly", "Future", "Index").forEach { n ->
                                        Action(label(n, ko), "nav-$n", n == logName) { navigate(n) }
                                    }
                                }
                        }
                        if (menu)
                            item {
                                Panel {
                                    Text(tr("언어", "Language"), fontWeight = FontWeight.Bold)
                                    FlowRow {
                                        Language.entries.forEach { l ->
                                            Action(
                                                if (l == Language.Korean) "한국어" else "English",
                                                "language-$l",
                                                l == state.settings.language,
                                            ) {
                                                vm.settings(state.settings.copy(language = l))
                                            }
                                        }
                                    }
                                    Text(tr("테마", "Theme"), fontWeight = FontWeight.Bold)
                                    FlowRow {
                                        ThemeMode.entries.forEach { t ->
                                            Action(
                                                when (t) {
                                                    ThemeMode.System -> tr("시스템", "System")
                                                    ThemeMode.Light -> tr("라이트", "Light")
                                                    ThemeMode.Dark -> tr("다크", "Dark")
                                                },
                                                "theme-$t",
                                                t == state.settings.theme,
                                            ) {
                                                vm.settings(state.settings.copy(theme = t))
                                            }
                                        }
                                    }
                                    Text(tr("컬렉션", "Collections"), fontWeight = FontWeight.Bold)
                                    state.journal.collections.forEach { c ->
                                        Action(c.name, "collection-${c.id}") {
                                            navigate(c.id.toString())
                                        }
                                    }
                                    OutlinedTextField(
                                        collectionName,
                                        { collectionName = it },
                                        enabled = !state.busy,
                                        keyboardOptions =
                                            KeyboardOptions(imeAction = ImeAction.Done),
                                        keyboardActions =
                                            KeyboardActions(onDone = { createCollection() }),
                                        label = { Text(tr("컬렉션 이름", "Collection name")) },
                                        modifier =
                                            Modifier.fillMaxWidth().testTag("collection-name"),
                                    )
                                    Action(
                                        tr("컬렉션 만들기", "Create collection"),
                                        "collection-create",
                                        enabled = !state.blocked && !state.busy,
                                    ) {
                                        createCollection()
                                    }
                                }
                            }
                        if (state.loading) item { LinearProgressIndicator(Modifier.fillMaxWidth()) }
                        state.error?.let { code ->
                            item {
                                Panel {
                                    Text(
                                        errorMessage(code),
                                        color = MaterialTheme.colorScheme.error,
                                    )
                                    if (!state.blocked)
                                        Action(tr("닫기", "Dismiss"), "dismiss-error") {
                                            vm.dismissError()
                                        }
                                }
                            }
                        }
                        item {
                            Text(
                                if (search.isNotBlank()) tr("검색 결과", "Search results")
                                else if (logName == "Index") label("Index", ko)
                                else if (log is Log.Collection)
                                    state.journal.collections.find { it.id == log.id }?.name ?: ""
                                else label(logName, ko),
                                style = MaterialTheme.typography.headlineLarge,
                            )
                            Text(
                                tr(
                                    "생각을 비우고, 중요한 일에 집중하세요.",
                                    "Clear your mind. Focus on what matters.",
                                ),
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        if (logName != "Index")
                            item {
                                FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                                    Action("‹", "previous") {
                                        try {
                                            dateText =
                                                if (log == Log.Monthly || log == Log.Future)
                                                    shiftMonth(date, -1).toString()
                                                else
                                                    date
                                                        .minusDays(1)
                                                        .also { parseDate(it.toString()) }
                                                        .toString()
                                        } catch (_: Exception) {
                                            vm.report("date")
                                        }
                                    }
                                    Action(tr("오늘", "Today"), "today") {
                                        dateText = LocalDate.now().toString()
                                    }
                                    Action("›", "next") {
                                        try {
                                            dateText =
                                                if (log == Log.Monthly || log == Log.Future)
                                                    shiftMonth(date, 1).toString()
                                                else
                                                    date
                                                        .plusDays(1)
                                                        .also { parseDate(it.toString()) }
                                                        .toString()
                                        } catch (_: Exception) {
                                            vm.report("date")
                                        }
                                    }
                                }
                                OutlinedTextField(
                                    dateText,
                                    { dateText = it },
                                    label = { Text(tr("날짜 YYYY-MM-DD", "Date YYYY-MM-DD")) },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth().testTag("date"),
                                )
                            }
                        item {
                            OutlinedTextField(
                                search,
                                { search = it },
                                label = { Text(tr("전체 기록 검색", "Search all entries")) },
                                modifier = Modifier.fillMaxWidth().testTag("search"),
                            )
                            FlowRow {
                                Filter.entries.forEach { f ->
                                    Action(
                                        when (f) {
                                            Filter.All -> tr("모든 기록", "All")
                                            Filter.Open -> tr("미완료", "Open")
                                            Filter.Complete -> tr("완료", "Complete")
                                        },
                                        "filter-$f",
                                        filter == f,
                                    ) {
                                        filterName = f.name
                                    }
                                }
                                if (search.isNotEmpty())
                                    Action(tr("검색 지우기", "Clear search"), "search-clear") {
                                        search = ""
                                    }
                            }
                        }
                        if (logName != "Index")
                            item {
                                Panel {
                                    FlowRow {
                                        Kind.entries.forEach { k ->
                                            Action(
                                                when (k) {
                                                    Kind.Task -> tr("• 할 일", "• Task")
                                                    Kind.Event -> tr("○ 이벤트", "○ Event")
                                                    Kind.Note -> tr("– 메모", "– Note")
                                                },
                                                "kind-$k",
                                                kindName == k.name,
                                            ) {
                                                kindName = k.name
                                            }
                                        }
                                    }
                                    OutlinedTextField(
                                        draft,
                                        { draft = it },
                                        enabled = !state.busy,
                                        keyboardOptions =
                                            KeyboardOptions(imeAction = ImeAction.Done),
                                        keyboardActions =
                                            KeyboardActions(
                                                onDone = {
                                                    if (draft.composition == null) capture()
                                                }
                                            ),
                                        label = {
                                            Text(
                                                tr("지금 떠오르는 생각을 기록하세요", "Write what's on your mind")
                                            )
                                        },
                                        modifier = Modifier.fillMaxWidth().testTag("draft"),
                                        minLines = 2,
                                    )
                                    Button(
                                        onClick = { capture() },
                                        enabled = !state.loading && !state.blocked && !state.busy,
                                        shape = RoundedCornerShape(8.dp),
                                        modifier =
                                            Modifier.fillMaxWidth()
                                                .heightIn(min = 48.dp)
                                                .testTag("capture"),
                                    ) {
                                        Text(tr("기록하기", "Capture"))
                                    }
                                }
                            }
                        if (log == Log.Monthly && logName != "Index" && search.isBlank())
                            item {
                                Panel {
                                    Text(
                                        tr("월간 달력", "Monthly calendar"),
                                        style = MaterialTheme.typography.titleLarge,
                                    )
                                    val month = YearMonth.from(date)
                                    FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                                        (1..month.lengthOfMonth()).forEach { day ->
                                            val d = month.atDay(day)
                                            val count = state.journal.visible(d, Log.Daily).size
                                            Action(
                                                "$day ${if(ko) listOf("월","화","수","목","금","토","일")[d.dayOfWeek.value-1] else d.dayOfWeek.name.take(3)} · $count",
                                                "day-$day",
                                            ) {
                                                dateText = d.toString()
                                                navigate("Daily")
                                            }
                                        }
                                    }
                                }
                            }
                        if (logName == "Index" && search.isBlank()) {
                            val locations =
                                state.journal.entries.distinctBy { e ->
                                    when (val l = e.log) {
                                        Log.Daily -> "D${e.date}"
                                        Log.Monthly -> "M${YearMonth.from(e.date)}"
                                        Log.Future -> "F${YearMonth.from(e.date)}"
                                        is Log.Collection -> l.id.toString()
                                    }
                                }
                            items(locations, key = { "index-${it.id}" }) { e ->
                                Action(
                                    "${location(e,state.journal,ko)} · ${e.date}",
                                    "index-${e.id}",
                                ) {
                                    jump(e)
                                }
                            }
                            items(state.journal.collections, key = { "index-c-${it.id}" }) { c ->
                                Action(c.name, "index-collection-${c.id}") {
                                    navigate(c.id.toString())
                                }
                            }
                        }
                        val visible =
                            if (search.isNotBlank()) state.journal.search(search, filter)
                            else if (logName == "Index") emptyList()
                            else
                                state.journal.visible(date, log).filter {
                                    matchesFilter(it, filter)
                                }
                        if (
                            visible.isEmpty() &&
                                (logName != "Index" || search.isNotBlank()) &&
                                !state.loading
                        )
                            item {
                                Text(
                                    if (search.isNotBlank())
                                        tr("검색 결과가 없습니다.", "No matching entries.")
                                    else
                                        tr(
                                            "아직 기록이 없습니다. 첫 생각을 남겨 보세요.",
                                            "No entries yet. Capture your first thought.",
                                        ),
                                    Modifier.padding(vertical = 20.dp),
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        items(visible, key = { it.id }) { e ->
                            Panel(Modifier.testTag("entry-${e.id}")) {
                                Text(
                                    "${if(e.important) "★ " else ""}${e.symbol()}  ${e.text}",
                                    style = MaterialTheme.typography.bodyLarge,
                                    color =
                                        if (e.important) MaterialTheme.colorScheme.primary
                                        else MaterialTheme.colorScheme.onSurface,
                                )
                                Text(
                                    "${e.date} · ${location(e,state.journal,ko)}",
                                    style = MaterialTheme.typography.bodySmall,
                                )
                                FlowRow {
                                    if (search.isNotBlank())
                                        Action(tr("위치로 이동", "Open location"), "jump-${e.id}") {
                                            jump(e)
                                        }
                                    if (!e.frozen()) {
                                        Action(
                                            tr("수정", "Edit"),
                                            "edit-${e.id}",
                                            enabled = !state.busy,
                                        ) {
                                            editId = e.id.toString()
                                            editText = e.text
                                        }
                                        Action(
                                            tr("중요", "Important"),
                                            "important-${e.id}",
                                            e.important,
                                            !state.busy,
                                        ) {
                                            vm.execute(JournalCommand.ToggleImportant(e.id))
                                        }
                                        if (e.kind == Kind.Task)
                                            Action(
                                                if (e.status == Status.Complete) tr("재개", "Reopen")
                                                else tr("완료", "Complete"),
                                                "complete-${e.id}",
                                                enabled = !state.busy,
                                            ) {
                                                vm.execute(
                                                    JournalCommand.SetStatus(
                                                        e.id,
                                                        if (e.status == Status.Complete) Status.Open
                                                        else Status.Complete,
                                                    )
                                                )
                                            }
                                        Action(
                                            if (e.status == Status.Cancelled) tr("재개", "Reopen")
                                            else tr("취소", "Cancel"),
                                            "cancel-${e.id}",
                                            enabled = !state.busy,
                                        ) {
                                            vm.execute(
                                                JournalCommand.SetStatus(
                                                    e.id,
                                                    if (e.status == Status.Cancelled) Status.Open
                                                    else Status.Cancelled,
                                                )
                                            )
                                        }
                                        if (e.isOpenTask())
                                            Action(
                                                tr("이월", "Migrate"),
                                                "migrate-${e.id}",
                                                enabled = !state.busy,
                                            ) {
                                                migrationId = e.id.toString()
                                                migrationLog = "Daily"
                                                migrationDate = date.plusDays(1).toString()
                                            }
                                    }
                                    e.migratedFrom?.let { id ->
                                        Action(tr("← 원본", "← Source"), "source-${e.id}") {
                                            jump(state.journal.entry(id))
                                        }
                                    }
                                    e.migratedTo?.let { id ->
                                        Action(tr("대상 →", "Target →"), "target-${e.id}") {
                                            jump(state.journal.entry(id))
                                        }
                                    }
                                }
                            }
                        }
                        item {
                            Panel {
                                Text(
                                    tr(
                                        "기록 ${visible.size} · 열린 할 일 ${visible.count { it.isOpenTask() }}",
                                        "${visible.size} entries · ${visible.count { it.isOpenTask() }} open tasks",
                                    ),
                                    fontWeight = FontWeight.Bold,
                                )
                                Text(
                                    tr(
                                        "• 할 일   ○ 이벤트   – 메모\n× 완료   ⊘ 취소   > 이월   < 예약   ★ 중요",
                                        "• Task   ○ Event   – Note\n× Complete   ⊘ Cancelled   > Migrated   < Scheduled   ★ Important",
                                    )
                                )
                            }
                        }
                    }
                }
            }
        }
        if (editId.isNotEmpty())
            AlertDialog(
                containerColor = MaterialTheme.colorScheme.surface,
                titleContentColor = MaterialTheme.colorScheme.onBackground,
                textContentColor = MaterialTheme.colorScheme.onSurface,
                shape = RoundedCornerShape(12.dp),
                tonalElevation = 0.dp,
                onDismissRequest = { if (!state.busy) editId = "" },
                title = { Text(tr("기록 수정", "Edit entry")) },
                text = {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        OutlinedTextField(
                            editText,
                            { editText = it },
                            enabled = !state.busy,
                            modifier = Modifier.testTag("edit-text"),
                        )
                        state.error?.let {
                            Text(errorMessage(it), color = MaterialTheme.colorScheme.error)
                        }
                    }
                },
                confirmButton = {
                    Action(tr("저장", "Save"), "edit-save", enabled = !state.busy) {
                        val id = UUID.fromString(editId)
                        val text = editText
                        vm.execute(JournalCommand.EditEntry(id, text)) {
                            if (editId == id.toString()) editId = ""
                        }
                    }
                },
                dismissButton = {
                    Action(tr("닫기", "Close"), "edit-close", enabled = !state.busy) { editId = "" }
                },
            )
        if (migrationId.isNotEmpty())
            AlertDialog(
                containerColor = MaterialTheme.colorScheme.surface,
                titleContentColor = MaterialTheme.colorScheme.onBackground,
                textContentColor = MaterialTheme.colorScheme.onSurface,
                shape = RoundedCornerShape(12.dp),
                tonalElevation = 0.dp,
                onDismissRequest = { if (!state.busy) migrationId = "" },
                title = { Text(tr("할 일 이월", "Migrate task")) },
                text = {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        FlowRow {
                            listOf("Daily", "Monthly", "Future").forEach { n ->
                                Action(label(n, ko), "migration-$n", migrationLog == n) {
                                    migrationLog = n
                                }
                            }
                        }
                        OutlinedTextField(
                            migrationDate,
                            { migrationDate = it },
                            label = { Text("YYYY-MM-DD") },
                            modifier = Modifier.testTag("migration-date"),
                        )
                        state.error?.let {
                            Text(errorMessage(it), color = MaterialTheme.colorScheme.error)
                        }
                    }
                },
                confirmButton = {
                    Action(tr("이월하기", "Migrate"), "migration-commit", enabled = !state.busy) {
                        try {
                            val d = parseDate(migrationDate)
                            val target =
                                when (migrationLog) {
                                    "Monthly" -> Log.Monthly
                                    "Future" -> Log.Future
                                    else -> Log.Daily
                                }
                            val id = UUID.fromString(migrationId)
                            vm.execute(JournalCommand.Migrate(id, d, target)) {
                                if (migrationId == id.toString()) migrationId = ""
                            }
                        } catch (_: Exception) {
                            vm.report("date")
                        }
                    }
                },
                dismissButton = {
                    Action(tr("닫기", "Close"), "migration-close", enabled = !state.busy) {
                        migrationId = ""
                    }
                },
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

@Composable
private fun Action(
    text: String,
    tag: String,
    selected: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    val primary = tag in setOf("collection-create", "edit-save", "migration-commit")
    val nav = tag.startsWith("nav-") || tag.startsWith("collection-")
    TextButton(
        onClick,
        enabled = enabled,
        modifier =
            Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp).testTag(tag).semantics {
                contentDescription =
                    when (tag) {
                        "previous" -> "이전 / Previous"
                        "next" -> "다음 / Next"
                        else -> text
                    }
            },
        shape = RoundedCornerShape(8.dp),
        colors =
            ButtonDefaults.textButtonColors(
                containerColor =
                    if (primary) MaterialTheme.colorScheme.primary
                    else if (selected) MaterialTheme.colorScheme.surfaceVariant
                    else androidx.compose.ui.graphics.Color.Transparent,
                contentColor =
                    if (primary) MaterialTheme.colorScheme.onPrimary
                    else if (nav && !selected) MaterialTheme.colorScheme.onSurfaceVariant
                    else MaterialTheme.colorScheme.onBackground,
                disabledContainerColor =
                    if (primary) MaterialTheme.colorScheme.surfaceVariant
                    else androidx.compose.ui.graphics.Color.Transparent,
                disabledContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
            ),
    ) {
        Text(text)
    }
}

@Composable
private fun Panel(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surface, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content,
    )
}
