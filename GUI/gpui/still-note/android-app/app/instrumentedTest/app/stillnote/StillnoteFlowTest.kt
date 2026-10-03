package app.stillnote

import android.content.Context
import android.graphics.Bitmap
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.dp
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.SavedStateHandle
import androidx.test.platform.app.InstrumentationRegistry
import app.stillnote.data.*
import app.stillnote.di.JournalDependencies
import app.stillnote.domain.*
import app.stillnote.presentation.JournalViewModel
import app.stillnote.ui.StillnoteApp
import java.io.File
import java.util.UUID
import org.junit.After
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

/** Runs in an empty test host; every case uses a unique cache directory. */
class StillnoteFlowTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    private val directory =
        File(
                InstrumentationRegistry.getInstrumentation().targetContext.cacheDir,
                "verification-${UUID.randomUUID()}",
            )
            .apply { mkdirs() }
    private lateinit var vm: JournalViewModel

    private fun configureHost() {
        compose.runOnUiThread {
            compose.activity.enableEdgeToEdge()
            compose.activity.window.setSoftInputMode(
                WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            )
        }
    }

    private fun start(
        savedState: SavedStateHandle = SavedStateHandle(),
        excludeNativeIme: Boolean = false,
    ) {
        configureHost()
        if (excludeNativeIme)
            compose.runOnUiThread {
                compose.activity.window.addFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
            }
        compose.runOnUiThread {
            vm = JournalViewModel(JournalDependencies.create(directory), savedState)
        }
        compose.setContent { StillnoteApp(vm) }
        compose.waitUntil(10_000) { !vm.state.value.loading }
        compose.waitForIdle()
    }

    private fun click(tag: String, scroll: Boolean = true) {
        if (!scroll)
            InstrumentationRegistry.getInstrumentation().uiAutomation.waitForIdle(500, 5_000)
        val node = if (scroll) visibleNode(tag) else compose.onNodeWithTag(tag)
        if (tag == "capture") {
            val insets = ViewCompat.getRootWindowInsets(compose.activity.window.decorView)
            println(
                "Capture pointer: bounds=${node.fetchSemanticsNode().boundsInRoot} root=${compose.onRoot().fetchSemanticsNode().boundsInRoot} ime=${insets?.getInsets(WindowInsetsCompat.Type.ime())} visible=${insets?.isVisible(WindowInsetsCompat.Type.ime())}"
            )
            val screenshot =
                File(
                    InstrumentationRegistry.getInstrumentation().targetContext.cacheDir,
                    "verification-capture-${UUID.randomUUID()}.png",
                )
            screenshot.outputStream().use {
                compose
                    .onRoot()
                    .captureToImage()
                    .asAndroidBitmap()
                    .compress(Bitmap.CompressFormat.PNG, 100, it)
            }
            println("Capture pointer screenshot=$screenshot")
        }
        node.assertIsDisplayed().performClick()
        compose.waitForIdle()
    }

    private fun enter(tag: String, text: String, scroll: Boolean = true) {
        val node = if (scroll) visibleNode(tag) else compose.onNodeWithTag(tag)
        node.assertIsDisplayed().performClick()
        compose.waitForIdle()
        if (
            scroll &&
                compose.activity.window.attributes.flags and
                    WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM == 0
        ) {
            // Text replacement focuses the field before the native keyboard has
            // necessarily resized the window. Wait for the real IME, then its
            // insets to stop changing before any scroll or physical pointer.
            var previousBottom = -1
            var stableSince = 0L
            compose.waitUntil(10_000) {
                val insets = ViewCompat.getRootWindowInsets(compose.activity.window.decorView)
                val bottom = insets?.getInsets(WindowInsetsCompat.Type.ime())?.bottom ?: 0
                val now = android.os.SystemClock.uptimeMillis()
                if (bottom != previousBottom) {
                    previousBottom = bottom
                    stableSince = now
                }
                insets?.isVisible(WindowInsetsCompat.Type.ime()) == true && now - stableSince >= 300
            }
            InstrumentationRegistry.getInstrumentation().uiAutomation.waitForIdle(500, 5_000)
            compose.waitForIdle()
        }
        node.performTextReplacement(text)
        compose.waitForIdle()
    }

    private fun visibleNode(tag: String): SemanticsNodeInteraction {
        // All visibility assertions and pointer actions need the settled native
        // window, including callers which do not go through click().
        InstrumentationRegistry.getInstrumentation().uiAutomation.waitForIdle(500, 5_000)
        if (
            tag == "menu" ||
                (tag.startsWith("nav-") &&
                    compose
                        .onAllNodesWithTag("fixed-navigation")
                        .fetchSemanticsNodes()
                        .isNotEmpty())
        ) {
            return compose.onNodeWithTag(tag).assertIsDisplayed()
        }
        val nodes = compose.onAllNodesWithTag(tag).fetchSemanticsNodes()
        // Lazy reuse can leave attached semantics with cached bounds even when
        // the item is not placed. Scroll the lazy list in that case as well.
        if (
            nodes.isEmpty() ||
                nodes.none { node ->
                    generateSequence(node.layoutInfo) { it.parentInfo }.all { it.isPlaced }
                }
        ) {
            compose
                .onNode(
                    hasScrollToIndexAction() and
                        SemanticsMatcher.keyIsDefined(SemanticsProperties.VerticalScrollAxisRange)
                )
                .performScrollToNode(hasTestTag(tag))
        }
        compose.onNodeWithTag(tag).performScrollTo()
        InstrumentationRegistry.getInstrumentation().uiAutomation.waitForIdle(500, 5_000)
        compose.waitForIdle()
        compose.onNodeWithTag(tag).performScrollTo()
        compose.waitForIdle()
        InstrumentationRegistry.getInstrumentation().uiAutomation.waitForIdle(500, 5_000)
        return compose.onNodeWithTag(tag)
    }

    private fun settled() {
        compose.waitUntil(10_000) { !vm.state.value.busy }
        compose.waitForIdle()
    }

    @Test
    fun ac10_imeCompositionProtocolDoesNotCaptureUntilFinished() {
        start(excludeNativeIme = true)
        visibleNode("draft").performClick()
        compose.waitForIdle()
        lateinit var connection: InputConnection
        fun find(view: View): InputConnection? {
            if (view.javaClass.name.contains("AndroidComposeView")) {
                view.onCreateInputConnection(EditorInfo())?.let {
                    return it
                }
            }
            if (view is ViewGroup) {
                for (index in 0 until view.childCount) find(view.getChildAt(index))?.let {
                    return it
                }
            }
            return null
        }
        compose.runOnUiThread {
            (compose.activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                .hideSoftInputFromWindow(compose.activity.window.decorView.windowToken, 0)
            connection = requireNotNull(find(compose.activity.window.decorView))
        }
        listOf("ㅎ", "하", "한").forEach { value ->
            compose.runOnUiThread {
                connection.beginBatchEdit()
                assertTrue(connection.setComposingText(value, 1))
                connection.endBatchEdit()
            }
            compose.waitForIdle()
        }
        compose.runOnUiThread { connection.performEditorAction(EditorInfo.IME_ACTION_DONE) }
        compose.waitForIdle()
        assertTrue(vm.state.value.journal.entries.isEmpty())
        compose.onNodeWithTag("draft").assertTextContains("한")
        compose.runOnUiThread { assertTrue(connection.finishComposingText()) }
        compose.waitForIdle()
        compose.runOnUiThread { connection.performEditorAction(EditorInfo.IME_ACTION_DONE) }
        compose.waitUntil(10_000) { vm.state.value.journal.entries.size == 1 }
        assertEquals("한", vm.state.value.journal.entries.single().text)
    }

    private fun capture(text: String, kind: Kind = Kind.Task): UUID {
        click("kind-$kind")
        enter("draft", text)
        compose
            .onNodeWithTag("draft")
            .assert(
                SemanticsMatcher.expectValue(
                    SemanticsProperties.EditableText,
                    AnnotatedString(text),
                )
            )
        visibleNode("capture").assertIsEnabled()
        click("capture")
        try {
            compose.waitUntil(10_000) {
                vm.state.value.journal.entries.any { it.text == text.trim() }
            }
        } catch (failure: Throwable) {
            throw AssertionError(
                "Capture expected=$text state=${vm.state.value} draft=${vm.recalled("draft")}",
                failure,
            )
        }
        settled()
        compose
            .onNodeWithTag("draft")
            .assert(
                SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString(""))
            )
        return vm.state.value.journal.entries.last().id
    }

    @After
    fun cleanUp() {
        directory.deleteRecursively()
    }

    @Test
    fun ac01_ac02_blankCaptureEditCompleteReopenCancelAndUnicode() {
        start()
        assertEquals(Journal(), vm.state.value.journal)
        assertFalse(File(directory, "journal.json").exists())
        enter("draft", " \t ")
        click("capture")
        compose.waitUntil(10_000) { vm.state.value.error == "text_empty" }
        assertTrue(vm.state.value.journal.entries.isEmpty())
        compose.onNodeWithTag("draft").assertTextContains(" \t ")
        val id = capture("한글 café 📚")
        click("important-$id")
        settled()
        assertTrue(vm.state.value.journal.entry(id).important)
        click("edit-$id")
        enter("edit-text", "수정 📝", false)
        click("edit-save", false)
        compose.waitUntil(10_000) { vm.state.value.journal.entry(id).text == "수정 📝" }
        click("complete-$id")
        settled()
        assertEquals("×", vm.state.value.journal.entry(id).symbol())
        click("complete-$id")
        settled()
        assertEquals(Status.Open, vm.state.value.journal.entry(id).status)
        click("cancel-$id")
        settled()
        assertEquals("⊘", vm.state.value.journal.entry(id).symbol())
        val event = capture("생일", Kind.Event)
        val note = capture("생각", Kind.Note)
        compose.onNodeWithTag("complete-$event").assertDoesNotExist()
        assertEquals("○", vm.state.value.journal.entry(event).symbol())
        assertEquals("–", vm.state.value.journal.entry(note).symbol())
        assertEquals(vm.state.value.journal, JournalStore(File(directory, "journal.json")).load())
    }

    @Test
    fun ac03_ac04_monthCalendarFutureCollectionIndexAndSearchLocation() {
        start()
        enter("date", "2024-02-29")
        val daily = capture("달력 기록")
        assertEquals(parseDate("2024-02-29"), vm.state.value.journal.entry(daily).date)
        // Follow the verified native flow: hide the keyboard before browsing the calendar.
        if (
            ViewCompat.getRootWindowInsets(compose.activity.window.decorView)
                ?.isVisible(WindowInsetsCompat.Type.ime()) == true
        ) {
            InstrumentationRegistry.getInstrumentation().sendKeyDownUpSync(KeyEvent.KEYCODE_BACK)
            compose.waitUntil(5_000) {
                ViewCompat.getRootWindowInsets(compose.activity.window.decorView)
                    ?.isVisible(WindowInsetsCompat.Type.ime()) != true
            }
        }
        click("nav-Monthly")
        // Dates now belong to each log; select the month containing the daily entry.
        visibleNode("date").performSemanticsAction(SemanticsActions.SetText) {
            assertTrue(it(AnnotatedString("2024-02-29")))
        }
        compose.waitForIdle()
        visibleNode("day-29").assertTextContains("· 1", substring = true)
        compose.onNodeWithTag("day-30").assertDoesNotExist()
        click("next")
        compose.onNodeWithTag("date").assertTextContains("2024-03-01")
        click("previous")
        compose.onNodeWithTag("date").assertTextContains("2024-02-01")
        val calendarNode = visibleNode("day-29").assertIsDisplayed().fetchSemanticsNode()
        val insets = ViewCompat.getRootWindowInsets(compose.activity.window.decorView)
        val diagnostics =
            "bounds=${calendarNode.boundsInRoot}, imeVisible=${insets?.isVisible(WindowInsetsCompat.Type.ime())}, imeInsets=${insets?.getInsets(WindowInsetsCompat.Type.ime())}, beforeDate=${vm.recalled("date")}, beforeLog=${vm.recalled("log")}, durableSource=${vm.state.value.journal.entry(daily).date}"
        val screenshot =
            File(
                InstrumentationRegistry.getInstrumentation().targetContext.cacheDir,
                "verification-calendar-${UUID.randomUUID()}.png",
            )
        screenshot.outputStream().use {
            compose
                .onRoot()
                .captureToImage()
                .asAndroidBitmap()
                .compress(Bitmap.CompressFormat.PNG, 100, it)
        }
        println("Calendar pointer diagnostic: $diagnostics screenshot=$screenshot")
        click("day-29")
        try {
            visibleNode("date").assertTextContains("2024-02-29")
        } catch (failure: Throwable) {
            throw AssertionError(
                "Calendar real pointer failed: $diagnostics afterDate=${vm.recalled("date")} afterLog=${vm.recalled("log")}",
                failure,
            )
        }
        visibleNode("entry-$daily").assertIsDisplayed()
        click("nav-Future")
        val future = capture("미래 기록")
        click("menu")
        enter("collection-name", "  독서 📚  ")
        click("collection-create")
        compose.waitUntil(10_000) { vm.state.value.journal.collections.isNotEmpty() }
        val collection = vm.state.value.journal.collections.single()
        compose.waitUntil(10_000) { vm.recalled("log") == collection.id.toString() }
        compose.onNodeWithTag("collection-name").assertDoesNotExist()
        // Creation selects the new collection and closes the menu. Reopen it to
        // exercise explicit collection selection as a separate user action.
        click("menu")
        click("collection-${collection.id}")
        val book = capture("읽은 책", Kind.Note)
        assertEquals(Log.Collection(collection.id), vm.state.value.journal.entry(book).log)
        click("nav-Index")
        click("index-$future")
        visibleNode("entry-$future").assertIsDisplayed()
        enter("search", "달력")
        click("jump-$daily")
        visibleNode("entry-$daily").assertIsDisplayed()
        visibleNode("date").assertTextContains("2024-02-29")
    }

    @Test
    fun ac05_searchAllLogsStatusFilterIncludesOpenEventsAndNotes() {
        start()
        val task = capture("책 TASK")
        click("complete-$task")
        settled()
        click("nav-Future")
        val note = capture("책 NOTE", Kind.Note)
        val event = capture("책 EVENT", Kind.Event)
        click("nav-Daily")
        enter("search", " 책 ")
        click("filter-Open")
        compose.onNodeWithTag("entry-$task").assertDoesNotExist()
        val noteNode = visibleNode("entry-$note")
        try {
            noteNode.assertIsDisplayed()
        } catch (failure: Throwable) {
            val insets = ViewCompat.getRootWindowInsets(compose.activity.window.decorView)
            val node = noteNode.fetchSemanticsNode()
            val placement =
                generateSequence(node.layoutInfo) { it.parentInfo }
                    .map {
                        "id=${it.semanticsId} placed=${it.isPlaced} attached=${it.isAttached} size=${it.width}x${it.height}"
                    }
                    .toList()
            val rootView = (node.root as androidx.compose.ui.platform.ViewRootForTest).view
            val rootRect = android.graphics.Rect()
            val rootVisible = rootView.getGlobalVisibleRect(rootRect)
            val diagnostics =
                "entryBounds=${node.boundsInRoot}, windowBounds=${node.boundsInWindow}, placement=$placement, rootShown=${rootView.isShown}, rootVisible=$rootVisible, globalRect=$rootRect, viewport=${compose.onNode(hasScrollToIndexAction() and SemanticsMatcher.keyIsDefined(SemanticsProperties.VerticalScrollAxisRange)).fetchSemanticsNode().boundsInRoot}, ime=${insets?.getInsets(WindowInsetsCompat.Type.ime())}, imeVisible=${insets?.isVisible(WindowInsetsCompat.Type.ime())}, log=${vm.recalled("log")}, search=${vm.recalled("search")}, filter=${vm.recalled("filter")}, entries=${vm.state.value.journal.entries}"
            val screenshot =
                File(
                    InstrumentationRegistry.getInstrumentation().targetContext.cacheDir,
                    "verification-search-${UUID.randomUUID()}.png",
                )
            screenshot.outputStream().use {
                compose
                    .onRoot()
                    .captureToImage()
                    .asAndroidBitmap()
                    .compress(Bitmap.CompressFormat.PNG, 100, it)
            }
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val values =
                android.content.ContentValues().apply {
                    put(android.provider.MediaStore.Images.Media.DISPLAY_NAME, screenshot.name)
                    put(android.provider.MediaStore.Images.Media.MIME_TYPE, "image/png")
                    put(
                        android.provider.MediaStore.Images.Media.RELATIVE_PATH,
                        "Pictures/StillnoteVerification",
                    )
                }
            val uri =
                context.contentResolver.insert(
                    android.provider.MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                    values,
                )
            requireNotNull(uri)
            context.contentResolver.openOutputStream(uri).use {
                requireNotNull(it).write(screenshot.readBytes())
            }
            throw AssertionError(
                "Search entry visibility failed: $diagnostics screenshot=$screenshot persistent=$uri",
                failure,
            )
        }
        visibleNode("entry-$event").assertIsDisplayed()
        click("filter-Complete")
        visibleNode("entry-$task").assertIsDisplayed()
        compose.onNodeWithTag("entry-$note").assertDoesNotExist()
        click("search-clear")
        click("filter-All")
        visibleNode("entry-$task").assertIsDisplayed()
        compose.onNodeWithTag("entry-$note").assertDoesNotExist()
    }

    @Test
    fun ac06_migrateAndNavigateBothLinksWithFrozenOriginal() {
        start()
        enter("date", "2026-10-03")
        val source = capture("이월할 일")
        click("important-$source")
        settled()
        click("migrate-$source")
        enter("migration-date", "2026-11-01", false)
        click("migration-Future", false)
        click("migration-commit", false)
        compose.waitUntil(10_000) { vm.state.value.journal.entries.size == 2 }
        val target = vm.state.value.journal.entry(source).migratedTo!!
        compose.onNodeWithTag("edit-$source").assertDoesNotExist()
        compose.onNodeWithTag("complete-$source").assertDoesNotExist()
        click("target-$source")
        visibleNode("entry-$target").assertIsDisplayed()
        assertTrue(vm.state.value.journal.entry(target).important)
        click("source-$target")
        visibleNode("date").assertTextContains("2026-10-03")
        visibleNode("entry-$source").assertIsDisplayed()
    }

    @Test
    fun ac08_saveConflictKeepsDraftAndDisplayedModel() {
        start()
        val id = capture("original")
        File(directory, "journal.json").writeText("external preserved")
        enter("draft", "draft retained")
        click("capture")
        compose.waitUntil(10_000) { vm.state.value.error == "journal_conflict" }
        compose.onNodeWithTag("draft").assertTextContains("draft retained")
        assertEquals(listOf(id), vm.state.value.journal.entries.map { it.id })
        assertEquals("external preserved", File(directory, "journal.json").readText())
    }

    @Test
    fun ac08_ac09_corruptSettingsDoesNotBlockJournalCorruptJournalBlocksCapture() {
        File(directory, "settings.json").writeText("settings corrupted")
        start()
        capture("journal available")
        assertEquals("settings corrupted", File(directory, "settings.json").readText())
    }

    @Test
    fun ac08_corruptJournalShowsRecoveryAndDisablesCapture() {
        File(directory, "journal.json").writeText("journal corrupted")
        start()
        visibleNode("capture").assertIsNotEnabled()
        assertTrue(vm.state.value.blocked)
        compose
            .onNodeWithText("journal.json", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        assertEquals("journal corrupted", File(directory, "journal.json").readText())
    }

    @Test
    fun ac09_ac10_liveLanguageThemePreserveDraftAndPersistSettings() {
        start()
        enter("draft", "작성 중 한글 café")
        enter("search", "search preserved")
        click("menu")
        click("language-English")
        click("theme-Dark")
        compose.waitUntil(10_000) {
            File(directory, "settings.json").exists() &&
                vm.state.value.settings == Settings(Language.English, ThemeMode.Dark)
        }
        visibleNode("draft").assertTextContains("작성 중 한글 café")
        visibleNode("search").assertTextContains("search preserved")
        compose.waitUntil(10_000) {
            try {
                SettingsStore(File(directory, "settings.json")).load() ==
                    Settings(Language.English, ThemeMode.Dark)
            } catch (_: Exception) {
                false
            }
        }
        compose.waitForIdle()
        assertEquals("작성 중 한글 café", vm.recalled("draft"))
        assertEquals("search preserved", vm.recalled("search"))
    }

    @Test
    fun ac10_savedInstanceRestorationRetainsDraftDateFilterAndSearch() {
        configureHost()
        compose.runOnUiThread { vm = JournalViewModel(JournalDependencies.create(directory)) }
        val restoration = StateRestorationTester(compose)
        restoration.setContent { StillnoteApp(vm) }
        compose.waitUntil(10_000) { !vm.state.value.loading }
        enter("date", "2024-02-29")
        enter("draft", "프로세스 복원 초안")
        enter("search", "restored search")
        click("filter-Open")
        compose.waitForIdle()
        restoration.emulateSavedInstanceStateRestore()
        visibleNode("date").assertTextContains("2024-02-29")
        visibleNode("draft").assertTextContains("프로세스 복원 초안")
        visibleNode("search").assertTextContains("restored search")
        assertEquals("Open", vm.recalled("filter"))
        assertEquals("프로세스 복원 초안", vm.recalled("draft"))
        assertFalse(File(directory, "journal.json").exists())
    }

    @Test
    fun ac10_recreatedViewModelRestoresSelectionAndEditMigrationFromSavedHandle() {
        configureHost()
        compose.runOnUiThread { vm = JournalViewModel(JournalDependencies.create(directory)) }
        val host = mutableStateOf(vm)
        compose.setContent { key(host.value) { StillnoteApp(host.value) } }
        compose.waitUntil(10_000) { !vm.state.value.loading }
        enter("date", "2024-02-29")
        val id = capture("original task")
        enter("draft", "한글 selection draft")
        compose.onNodeWithTag("draft").performTextInputSelection(TextRange(2, 7))
        compose.waitForIdle()
        compose
            .onNodeWithTag("draft")
            .assert(
                SemanticsMatcher.expectValue(
                    SemanticsProperties.TextSelectionRange,
                    TextRange(2, 7),
                )
            )
        var expectedSelection = TextRange(2, 7)
        fun recreate() {
            val values = vm.savedState.keys().associateWith { vm.savedState.get<Any?>(it) }
            assertEquals(expectedSelection.start.toString(), values["draft-cursor"])
            assertEquals(expectedSelection.end.toString(), values["draft-end"])
            compose.runOnUiThread {
                vm =
                    JournalViewModel(
                        JournalDependencies.create(directory),
                        SavedStateHandle(values),
                    )
                host.value = vm
            }
            compose.waitUntil(10_000) { !vm.state.value.loading }
            compose.waitForIdle()
        }
        recreate()
        visibleNode("draft")
            .assert(
                SemanticsMatcher.expectValue(
                    SemanticsProperties.TextSelectionRange,
                    TextRange(2, 7),
                )
            )
        enter("search", "original")
        click("filter-Open")
        click("edit-$id")
        enter("edit-text", "unfinished edit", false)
        compose.waitForIdle()
        expectedSelection =
            TextRange(vm.recalled("draft-cursor").toInt(), vm.recalled("draft-end").toInt())
        recreate()
        compose.onNodeWithTag("edit-text").assertTextContains("unfinished edit")
        click("edit-close", false)
        visibleNode("date").assertTextContains("2024-02-29")
        visibleNode("draft").assertTextContains("한글 selection draft")
        compose
            .onNodeWithTag("draft")
            .assert(
                SemanticsMatcher.expectValue(
                    SemanticsProperties.TextSelectionRange,
                    expectedSelection,
                )
            )
        visibleNode("search").assertTextContains("original")
        assertEquals("Open", vm.recalled("filter"))
        click("migrate-$id")
        enter("migration-date", "2025-01-01", false)
        click("migration-Future", false)
        compose.waitForIdle()
        expectedSelection =
            TextRange(vm.recalled("draft-cursor").toInt(), vm.recalled("draft-end").toInt())
        recreate()
        compose.onNodeWithTag("migration-date").assertTextContains("2025-01-01")
        assertEquals("Future", vm.recalled("migration-log"))
        assertEquals(id.toString(), vm.recalled("migration-id"))
        assertEquals(listOf(id), vm.state.value.journal.entries.map { it.id })
    }

    @Test
    fun ac11_backClosesTransientUiAndPrimaryTargetsMeet48Dp() {
        start()
        listOf("menu", "capture", "kind-Task", "previous", "next").forEach { tag ->
            visibleNode(tag).assertHeightIsAtLeast(48.dp).assertWidthIsAtLeast(48.dp)
        }
        val id = capture("back navigation")
        click("edit-$id")
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        compose.onNodeWithTag("edit-text").assertDoesNotExist()
        click("migrate-$id")
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        compose.onNodeWithTag("migration-date").assertDoesNotExist()
        enter("search", "back navigation")
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        compose.onNodeWithTag("search-clear").assertDoesNotExist()
        click("menu")
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        compose.onNodeWithTag("language-English").assertDoesNotExist()
    }
}
