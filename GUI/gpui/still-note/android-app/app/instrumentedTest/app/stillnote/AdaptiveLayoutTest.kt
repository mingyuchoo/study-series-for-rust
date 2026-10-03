package app.stillnote

import android.graphics.Bitmap
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
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

@OptIn(ExperimentalTestApi::class)
class AdaptiveLayoutTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    private val directory =
        File(
                InstrumentationRegistry.getInstrumentation().targetContext.cacheDir,
                "layout-${UUID.randomUUID()}",
            )
            .apply { mkdirs() }
    private lateinit var vm: JournalViewModel
    private val lastCollection = UUID(0, 118)

    private fun start(size: DpSize) {
        var journal = Journal()
        (1L..18L).forEach {
            journal =
                journal.addCollection("긴 컬렉션 이름 $it · Long collection title", UUID(0, 100 + it))
        }
        val store = JournalStore(File(directory, "journal.json"))
        store.load()
        store.save(journal)
        compose.runOnUiThread {
            compose.activity.enableEdgeToEdge()
            compose.activity.window.setSoftInputMode(
                WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            )
            compose.activity.window.addFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
            vm = JournalViewModel(JournalDependencies.create(directory))
        }
        compose.setContent {
            DeviceConfigurationOverride(
                DeviceConfigurationOverride.ForcedSize(size)
                    .then(DeviceConfigurationOverride.FontScale(1.5f))
            ) {
                StillnoteApp(vm)
            }
        }
        compose.waitUntil(10_000) { !vm.state.value.loading }
        compose.waitForIdle()
    }

    private fun click(tag: String, scroll: Boolean = true) {
        val node = if (scroll) visibleNode(tag) else compose.onNodeWithTag(tag)
        node.performClick()
        compose.waitForIdle()
    }

    private fun text(tag: String, value: String, scroll: Boolean = true) {
        val node = if (scroll) visibleNode(tag) else compose.onNodeWithTag(tag)
        // Accessibility SetText invokes the same field callback without the test library's
        // forced focus/physical keyboard, which cannot represent this artificial window.
        node.performSemanticsAction(SemanticsActions.SetText) {
            assertTrue(it(AnnotatedString(value)))
        }
        compose.waitForIdle()
    }

    private fun visibleNode(tag: String): SemanticsNodeInteraction {
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

    private fun captureLongAndEdit() {
        val content = "긴 한글 기록 café 📚 ".repeat(30)
        text("draft", content)
        click("capture")
        try {
            compose.waitUntil(10_000) { vm.state.value.journal.entries.size == 1 }
        } catch (failure: Throwable) {
            throw AssertionError(
                "Expanded capture state=${vm.state.value} draft=${vm.recalled("draft")}",
                failure,
            )
        }
        compose.waitForIdle()
        val entry = vm.state.value.journal.entries.single()
        assertEquals(Log.Collection(lastCollection), entry.log)
        click("edit-${entry.id}")
        text("edit-text", "수정한 긴 기록 ".repeat(30), false)
        compose.onNodeWithTag("edit-save").assertIsDisplayed().performClick()
        compose.waitUntil(10_000) { vm.state.value.journal.entry(entry.id).text.startsWith("수정한") }
    }

    private fun changeBothLocalesAndPalettes() {
        click("menu")
        click("language-English")
        click("theme-Light")
        compose.waitUntil(10_000) {
            vm.state.value.settings == Settings(Language.English, ThemeMode.Light)
        }
        click("language-Korean")
        click("theme-Dark")
        compose.waitUntil(10_000) {
            vm.state.value.settings == Settings(Language.Korean, ThemeMode.Dark)
        }
        click("menu")
    }

    @After
    fun cleanup() {
        directory.deleteRecursively()
    }

    @Test
    fun ac11_compactShortLargeFontCollectionsDialogAndBothThemesLocales() {
        start(DpSize(320.dp, 480.dp))
        click("menu")
        click("collection-$lastCollection")
        captureLongAndEdit()
        changeBothLocalesAndPalettes()
        visibleNode("capture").assertIsDisplayed()
    }

    @Test
    fun ac11_expandedShortLargeFontSidebarDialogAndBothThemesLocales() {
        start(DpSize(1000.dp, 360.dp))
        click("collection-$lastCollection")
        captureLongAndEdit()
        changeBothLocalesAndPalettes()
        visibleNode("capture").assertIsDisplayed()
        visibleNode("nav-Monthly").assertIsDisplayed().performClick()
        val day = visibleNode("day-1")
        println(
            "Expanded calendar: day=${day.fetchSemanticsNode().boundsInRoot} viewport=${compose.onNode(hasScrollToIndexAction() and SemanticsMatcher.keyIsDefined(SemanticsProperties.VerticalScrollAxisRange)).fetchSemanticsNode().boundsInRoot} root=${compose.onRoot().fetchSemanticsNode().boundsInRoot}"
        )
        val screenshot =
            File(
                InstrumentationRegistry.getInstrumentation().targetContext.cacheDir,
                "verification-expanded-${UUID.randomUUID()}.png",
            )
        screenshot.outputStream().use {
            compose
                .onRoot()
                .captureToImage()
                .asAndroidBitmap()
                .compress(Bitmap.CompressFormat.PNG, 100, it)
        }
        println("Expanded calendar screenshot=$screenshot")
        day.assertIsDisplayed()
    }
}
