package app.stillnote.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import app.stillnote.application.*
import app.stillnote.domain.*
import app.stillnote.presentation.*
import java.time.LocalDate
import java.util.UUID

@Composable
private fun saved(memory: ScreenMemory, key: String, default: String = ""): MutableState<String> {
    val state = rememberSaveable(key) { mutableStateOf(memory.recalled(key, default)) }
    LaunchedEffect(state.value) { memory.remember(key, state.value) }
    return state
}

@Composable
fun StillnoteScreen(
    state: UiState,
    actions: JournalScreenActions,
    memory: ScreenMemory,
    today: () -> LocalDate,
) {
    val strings = JournalStrings(state.settings.language)
    fun tr(k: String, e: String) = strings.text(k, e)
    val basicLogs = remember { JournalPage.basic.map { it.key } }
    val initialLog = remember { memory.recalled("log", "Daily") }
    val pager =
        rememberPagerState(initialPage = basicLogs.indexOf(initialLog).coerceAtLeast(0)) {
            basicLogs.size
        }
    var collectionLog by rememberSaveable {
        mutableStateOf(initialLog.takeUnless { it in basicLogs } ?: "")
    }
    val logName = collectionLog.ifEmpty { basicLogs[pager.currentPage] }
    val pageNames =
        (basicLogs +
                state.journal.collections.map { it.id.toString() } +
                listOfNotNull(collectionLog.takeIf { it.isNotEmpty() }))
            .distinct()
    val dates =
        pageNames.associateWith { name ->
            key(name) { saved(memory, "date-$name", memory.recalled("date", today().toString())) }
        }
    val validDates =
        pageNames.associateWith { name ->
            key(name) {
                saved(
                    memory,
                    "selected-date-$name",
                    memory.recalled("selected-date", today().toString()),
                )
            }
        }
    val listStates =
        pageNames.associateWith { name ->
            key(name) {
                val list =
                    rememberSaveable(saver = LazyListState.Saver) {
                        LazyListState(
                            memory
                                .recalled("scroll-index-$name", "0")
                                .toIntOrNull()
                                ?.coerceAtLeast(0) ?: 0,
                            memory
                                .recalled("scroll-offset-$name", "0")
                                .toIntOrNull()
                                ?.coerceAtLeast(0) ?: 0,
                        )
                    }
                LaunchedEffect(list) {
                    snapshotFlow { list.firstVisibleItemIndex to list.firstVisibleItemScrollOffset }
                        .collect { (index, offset) ->
                            memory.remember("scroll-index-$name", index.toString())
                            memory.remember("scroll-offset-$name", offset.toString())
                        }
                }
                list
            }
        }
    // A collection can still be restoring while the journal is loading.
    val fallbackDate = saved(memory, "date", today().toString())
    var dateText by dates.getOrElse(logName) { fallbackDate }
    var search by saved(memory, "search")
    var filterName by saved(memory, "filter", "All")
    var kindName by saved(memory, "kind", "Task")
    var futureCapture by saved(memory, "future-capture", "false")
    val focusManager = LocalFocusManager.current
    val keyboard = LocalSoftwareKeyboardController.current
    var menu by rememberSaveable { mutableStateOf(false) }
    var collectionName by saved(memory, "collection-name")
    var editId by saved(memory, "edit-id")
    var editText by saved(memory, "edit-text")
    var migrationId by saved(memory, "migration-id")
    var migrationDate by saved(memory, "migration-date", today().plusDays(1).toString())
    var migrationLog by saved(memory, "migration-log", "Daily")
    var draft by
        rememberSaveable(stateSaver = TextFieldValue.Saver) {
            mutableStateOf(
                TextFieldValue(
                    memory.recalled("draft"),
                    TextRange(
                        memory.recalled("draft-cursor", "0").toIntOrNull() ?: 0,
                        memory.recalled("draft-end", "0").toIntOrNull() ?: 0,
                    ),
                )
            )
        }
    LaunchedEffect(draft) {
        memory.remember("draft", draft.text)
        memory.remember("draft-cursor", draft.selection.start.toString())
        memory.remember("draft-end", draft.selection.end.toString())
    }
    val fallbackSelectedDate = saved(memory, "selected-date", today().toString())
    var selectedDate by validDates.getOrElse(logName) { fallbackSelectedDate }
    LaunchedEffect(logName, dateText) {
        try {
            selectedDate = parseDate(dateText).toString()
        } catch (_: DomainException) {}
        memory.remember("date", dateText)
        memory.remember("selected-date", selectedDate)
    }
    fun pageLog(name: String): Log = JournalPage.fromKey(name).log
    val filter = Filter.valueOf(filterName)
    var previousLog by remember { mutableStateOf(logName) }
    LaunchedEffect(logName) {
        memory.remember("log", logName)
        if (previousLog != logName) {
            search = ""
            previousLog = logName
        }
    }
    fun navigate(name: String) {
        search = ""
        editId = ""
        menu = false
        val page = basicLogs.indexOf(name)
        if (page >= 0) {
            pager.requestScrollToPage(page)
            collectionLog = ""
        } else collectionLog = name
    }
    fun jump(e: Entry) {
        val target = JournalPage.forLog(e.log).key
        dates.getValue(target).value = e.date.toString()
        navigate(target)
    }
    fun capture(pageName: String) {
        try {
            val d = parseDate(dates.getValue(pageName).value)
            val text = draft.text
            val kind = Kind.valueOf(kindName)
            actions.execute(JournalCommand.AddEntry(d, pageLog(pageName), kind, text)) {
                draft = TextFieldValue()
                if (pageName == "Future") {
                    futureCapture = "false"
                    focusManager.clearFocus()
                    keyboard?.hide()
                }
            }
        } catch (_: DomainException) {
            actions.report("date")
        }
    }
    fun createCollection() {
        val submitted = collectionName
        actions.execute(JournalCommand.AddCollection(submitted)) { committed ->
            collectionName = ""
            navigate(committed.collections.last().id.toString())
        }
    }
    BackHandler(menu || editId.isNotEmpty() || migrationId.isNotEmpty() || search.isNotEmpty()) {
        when {
            menu -> menu = false
            migrationId.isNotEmpty() -> migrationId = ""
            editId.isNotEmpty() -> editId = ""
            else -> search = ""
        }
    }
    @Composable
    fun LogPage(pageName: String, expanded: Boolean, modifier: Modifier) {
        val logName = pageName
        var dateText by dates.getValue(pageName)
        val date =
            try {
                parseDate(dateText)
            } catch (_: DomainException) {
                parseDate(validDates.getValue(pageName).value)
            }
        val page = JournalPage.fromKey(pageName)
        val log = page.log
        val model = LogPageModel.project(state.journal, page, date, search, filter)
        LazyColumn(
            modifier
                .fillMaxHeight()
                .testTag("log-list-$pageName")
                .padding(horizontal = if (expanded) 28.dp else 16.dp),
            state = listStates.getValue(pageName),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            contentPadding = PaddingValues(top = 12.dp, bottom = 24.dp),
        ) {
            if (menu)
                item {
                    JournalSettingsPanel(
                        state.settings,
                        state.journal.collections,
                        collectionName,
                        state.busy,
                        state.blocked,
                        strings,
                        actions::settings,
                        ::navigate,
                        { collectionName = it },
                        ::createCollection,
                    )
                }
            if (state.loading) item { LinearProgressIndicator(Modifier.fillMaxWidth()) }
            item {
                Text(
                    if (logName == "Index") strings.pageLabel("Index")
                    else if (log is Log.Collection)
                        state.journal.collections.find { it.id == log.id }?.name ?: ""
                    else strings.pageLabel(logName),
                    style = MaterialTheme.typography.headlineLarge,
                )
                Text(
                    tr("생각을 비우고, 중요한 일에 집중하세요.", "Clear your mind. Focus on what matters."),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (logName != "Index")
                item {
                    DateNavigationPanel(
                        dateText,
                        state.error,
                        strings,
                        { dateText = it },
                        { delta ->
                            try {
                                dateText = page.shiftDate(date, delta).toString()
                            } catch (_: DomainException) {
                                actions.report("date")
                            }
                        },
                        { dateText = today().toString() },
                    )
                }
            if (model.calendar.isNotEmpty())
                item {
                    MonthlyCalendarPanel(
                        model.calendar,
                        model.date,
                        state.settings.language,
                        strings,
                    ) { selected ->
                        dates.getValue("Daily").value = selected.toString()
                        navigate("Daily")
                    }
                }
            state.error
                ?.takeUnless { logName != "Index" && it in setOf("date", "text_empty") }
                ?.let { code ->
                    item {
                        Panel {
                            Text(
                                strings.errorMessage(code),
                                color = MaterialTheme.colorScheme.error,
                            )
                            if (!state.blocked)
                                Action(tr("닫기", "Dismiss"), "dismiss-error") {
                                    actions.dismissError()
                                }
                        }
                    }
                }
            if (log == Log.Future)
                item {
                    Action(
                        if (futureCapture == "true") tr("입력란 접기", "Hide entry form")
                        else tr("기록 추가", "Add entry"),
                        "future-capture-toggle",
                        selected = futureCapture == "true",
                        enabled = !state.loading && !state.busy,
                    ) {
                        futureCapture = (futureCapture != "true").toString()
                        focusManager.clearFocus()
                        keyboard?.hide()
                    }
                }
            if (logName != "Index" && (log != Log.Future || futureCapture == "true"))
                item {
                    EntryCapturePanel(
                        draft,
                        Kind.valueOf(kindName),
                        state.busy,
                        !state.loading && !state.blocked && !state.busy,
                        state.error,
                        strings,
                        { draft = it },
                        { kindName = it.name },
                        { capture(pageName) },
                    )
                }
            item {
                JournalSearchPanel(
                    search,
                    filter,
                    strings,
                    { search = it },
                    { filterName = it.name },
                )
            }
            if (logName == "Index" && search.isBlank()) {
                val locations = model.indexLocations
                items(locations, key = { "index-${it.id}" }) { e ->
                    Action("${strings.location(e,state.journal)} · ${e.date}", "index-${e.id}") {
                        jump(e)
                    }
                }
                items(state.journal.collections, key = { "index-c-${it.id}" }) { c ->
                    Action(c.name, "index-collection-${c.id}") { navigate(c.id.toString()) }
                }
            }
            val visible = model.entries
            if (visible.isEmpty() && (logName != "Index" || search.isNotBlank()) && !state.loading)
                item {
                    Text(
                        if (search.isNotBlank()) tr("검색 결과가 없습니다.", "No matching entries.")
                        else
                            tr(
                                "아직 기록이 없습니다. 첫 생각을 남겨 보세요.",
                                "No entries yet. Capture your first thought.",
                            ),
                        Modifier.padding(vertical = 20.dp),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            items(visible, key = { it.id }) { entry ->
                EntryCard(
                    entry,
                    strings.location(entry, state.journal),
                    model.searching,
                    state.busy,
                    strings,
                ) { intent ->
                    when (intent) {
                        is EntryIntent.Open -> jump(state.journal.entry(intent.id))
                        is EntryIntent.Edit -> {
                            editId = intent.id.toString()
                            editText = state.journal.entry(intent.id).text
                        }
                        is EntryIntent.ToggleImportant ->
                            actions.execute(JournalCommand.ToggleImportant(intent.id))
                        is EntryIntent.SetStatus ->
                            actions.execute(JournalCommand.SetStatus(intent.id, intent.status))
                        is EntryIntent.Migrate -> {
                            migrationId = intent.id.toString()
                            migrationLog = "Daily"
                            migrationDate = date.plusDays(1).toString()
                        }
                    }
                }
            }
            item {
                Panel {
                    Text(
                        tr(
                            "기록 ${visible.size} · 열린 할 일 ${model.openTaskCount}",
                            "${visible.size} entries · ${model.openTaskCount} open tasks",
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
                            basicLogs.forEach { name ->
                                Action(strings.pageLabel(name), "nav-$name", name == logName) {
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
                    Column(Modifier.weight(1f).fillMaxHeight()) {
                        Column(
                            Modifier.fillMaxWidth()
                                .testTag("fixed-header")
                                .padding(horizontal = if (expanded) 28.dp else 16.dp)
                        ) {
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
                                FlowRow(
                                    modifier = Modifier.testTag("fixed-navigation"),
                                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                                ) {
                                    basicLogs.forEach { name ->
                                        Action(
                                            strings.pageLabel(name),
                                            "nav-$name",
                                            name == logName,
                                        ) {
                                            navigate(name)
                                        }
                                    }
                                }
                        }
                        if (collectionLog.isNotEmpty()) {
                            if (collectionLog in dates)
                                LogPage(collectionLog, expanded, Modifier.weight(1f))
                        } else {
                            HorizontalPager(
                                state = pager,
                                modifier = Modifier.weight(1f).fillMaxHeight().testTag("log-pager"),
                                key = { basicLogs[it] },
                                userScrollEnabled =
                                    !menu &&
                                        editId.isEmpty() &&
                                        migrationId.isEmpty() &&
                                        !state.busy,
                            ) { page ->
                                Box(
                                    Modifier.fillMaxSize()
                                        .then(
                                            if (page == pager.currentPage) Modifier
                                            else Modifier.clearAndSetSemantics {}
                                        )
                                ) {
                                    LogPage(basicLogs[page], expanded, Modifier.fillMaxSize())
                                }
                            }
                        }
                    }
                }
            }
        }
        if (editId.isNotEmpty())
            EditEntryDialog(
                editText,
                state.busy,
                state.error,
                strings,
                { editText = it },
                {
                    val id = UUID.fromString(editId)
                    actions.execute(JournalCommand.EditEntry(id, editText)) {
                        if (editId == id.toString()) editId = ""
                    }
                },
                { editId = "" },
            )
        if (migrationId.isNotEmpty())
            MigrateEntryDialog(
                migrationDate,
                migrationLog,
                state.busy,
                state.error,
                strings,
                { migrationDate = it },
                { migrationLog = it },
                {
                    try {
                        val date = parseDate(migrationDate)
                        val target = JournalPage.fromKey(migrationLog).log
                        val id = UUID.fromString(migrationId)
                        actions.execute(JournalCommand.Migrate(id, date, target)) {
                            if (migrationId == id.toString()) migrationId = ""
                        }
                    } catch (_: DomainException) {
                        actions.report("date")
                    }
                },
                { migrationId = "" },
            )
    }
}
