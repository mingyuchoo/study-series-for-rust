package app.stillnote

import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import app.stillnote.application.JournalCommand
import app.stillnote.domain.*
import app.stillnote.presentation.JournalScreenActions
import app.stillnote.presentation.ScreenMemory
import app.stillnote.presentation.UiState
import app.stillnote.ui.StillnoteScreen
import java.time.LocalDate
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class LogPagerTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    private val values = mutableMapOf<String, String>()
    private val memory =
        object : ScreenMemory {
            override fun remember(key: String, value: String) {
                values[key] = value
            }

            override fun recalled(key: String, default: String) = values[key] ?: default
        }
    private val actions =
        object : JournalScreenActions {
            override fun execute(command: JournalCommand, onSuccess: (Journal) -> Unit) {}

            override fun settings(settings: Settings) {}

            override fun report(error: String) {}

            override fun dismissError() {}
        }
    private val today = LocalDate.of(2026, 10, 4)
    private val collectionId = UUID(0, 100)
    private val entryId = UUID(0, 1)

    private fun start(journal: Journal = Journal()): StateRestorationTester {
        compose.runOnUiThread {
            compose.activity.window.addFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
        }
        val restoration = StateRestorationTester(compose)
        restoration.setContent {
            StillnoteScreen(UiState(journal = journal, loading = false), actions, memory) { today }
        }
        return restoration
    }

    private fun swipe(name: String, forward: Boolean) {
        compose.onNodeWithTag("log-list-$name").performTouchInput {
            val y = height * 0.15f
            val left = Offset(width * 0.2f, y)
            val right = Offset(width * 0.8f, y)
            swipe(if (forward) right else left, if (forward) left else right, 400)
        }
        compose.waitForIdle()
    }

    private fun selected(name: String) {
        compose.onNodeWithTag("nav-$name").assertIsSelected()
        assertEquals(name, values["log"])
    }

    @Test
    fun headerAndNavigationStayFixedWhileOnlyBodyMoves() {
        start()
        val header = compose.onNodeWithTag("fixed-header").fetchSemanticsNode().boundsInRoot
        val tabs =
            listOf("Daily", "Monthly", "Future", "Index").associateWith {
                compose.onNodeWithTag("nav-$it").fetchSemanticsNode().boundsInRoot
            }
        val body = compose.onNodeWithTag("log-pager").fetchSemanticsNode().boundsInRoot
        assertTrue(body.top >= header.bottom)
        compose.onNodeWithTag("fixed-header").performTouchInput { swipeLeft() }
        selected("Daily")
        compose.onNodeWithTag("fixed-navigation").performTouchInput { swipeLeft() }
        selected("Daily")
        compose.onNodeWithTag("log-pager").performTouchInput {
            down(Offset(width * 0.8f, height * 0.08f))
            moveTo(Offset(width * 0.5f, height * 0.08f), 200)
        }
        assertEquals(
            header,
            compose.onNodeWithTag("fixed-header").fetchSemanticsNode().boundsInRoot,
        )
        tabs.forEach { (name, bounds) ->
            assertEquals(
                bounds,
                compose.onNodeWithTag("nav-$name").fetchSemanticsNode().boundsInRoot,
            )
        }
        compose.onNodeWithTag("log-pager").performTouchInput {
            moveTo(Offset(width * 0.2f, height * 0.08f), 200)
            up()
        }
        compose.waitForIdle()
        selected("Monthly")
        compose.onNodeWithTag("log-list-Monthly").performScrollToIndex(3)
        assertEquals(
            header,
            compose.onNodeWithTag("fixed-header").fetchSemanticsNode().boundsInRoot,
        )
        tabs.forEach { (name, bounds) ->
            assertEquals(
                bounds,
                compose.onNodeWithTag("nav-$name").fetchSemanticsNode().boundsInRoot,
            )
        }
    }

    @Test
    fun swipesFollowMenuOrderAndStopAtBothEnds() {
        start()
        swipe("Daily", false)
        selected("Daily")
        swipe("Daily", true)
        selected("Monthly")
        swipe("Monthly", true)
        selected("Future")
        swipe("Future", true)
        selected("Index")
        swipe("Index", true)
        selected("Index")
        swipe("Index", false)
        selected("Future")
        compose.onNodeWithTag("nav-Daily").performClick()
        selected("Daily")
        compose.onNodeWithTag("nav-Index").performClick()
        selected("Index")
    }

    @Test
    fun datesAndScrollPositionsSurviveSwitchingAndRestoration() {
        val restoration =
            start(
                Journal(
                    entries =
                        (1..30).map { n ->
                            Entry(
                                UUID(0, n.toLong()),
                                LocalDate.of(2025, 6, 1),
                                Log.Monthly,
                                Kind.Note,
                                text = "Monthly note $n",
                            )
                        }
                )
            )
        compose.onNodeWithTag("date").performScrollTo().performTextReplacement("2024-02-29")
        swipe("Daily", true)
        compose.onNodeWithTag("date").performScrollTo().assertTextContains("2026-10-04")
        compose.onNodeWithTag("date").performTextReplacement("2025-06-01")
        compose.onNodeWithTag("log-list-Monthly").performScrollToIndex(5)
        compose.waitForIdle()
        val index = values["scroll-index-Monthly"]
        val offset = values["scroll-offset-Monthly"]
        swipe("Monthly", false)
        compose.onNodeWithTag("date").performScrollTo().assertTextContains("2024-02-29")
        swipe("Daily", true)
        assertEquals(index, values["scroll-index-Monthly"])
        assertEquals(offset, values["scroll-offset-Monthly"])
        restoration.emulateSavedInstanceStateRestore()
        assertEquals("Monthly", values["log"])
        assertEquals(index, values["scroll-index-Monthly"])
        compose.onNodeWithTag("log-list-Monthly").performScrollToNode(hasTestTag("date"))
        compose.onNodeWithTag("date").performScrollTo().assertTextContains("2025-06-01")
    }

    @Test
    fun settingsAndCollectionsDoNotSwipeToAnotherLog() {
        start(Journal(collections = listOf(Collection(collectionId, "Reading"))))
        compose.onNodeWithTag("menu").performClick()
        swipe("Daily", true)
        selected("Daily")
        compose.onNodeWithTag("collection-$collectionId").performScrollTo().performClick()
        compose.onNodeWithTag("log-pager").assertDoesNotExist()
        swipe(collectionId.toString(), true)
        assertEquals(collectionId.toString(), values["log"])
        compose.onNodeWithTag("nav-Monthly").performClick()
        selected("Monthly")
        swipe("Monthly", true)
        selected("Future")
    }

    @Test
    fun restoredCollectionCanReturnDirectlyToAnyBasicPage() {
        values["log"] = collectionId.toString()
        start(Journal(collections = listOf(Collection(collectionId, "Reading"))))
        compose.onNodeWithTag("log-pager").assertDoesNotExist()
        compose.onNodeWithTag("nav-Future").performClick()
        compose.waitForIdle()
        selected("Future")
        swipe("Future", false)
        selected("Monthly")
    }

    @Test
    fun futureEntryFormIsOptionalAndRetainsDraftAcrossCollapseAndRestoration() {
        val restoration = start()
        compose.onNodeWithTag("nav-Future").performClick()
        compose.onNodeWithTag("draft").assertDoesNotExist()
        compose.onNodeWithTag("search").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("future-capture-toggle").performScrollTo().performClick()
        compose.onNodeWithTag("draft").performScrollTo().performTextReplacement("Future plan")
        compose.onNodeWithTag("future-capture-toggle").performScrollTo().performClick()
        compose.onNodeWithTag("draft").assertDoesNotExist()
        restoration.emulateSavedInstanceStateRestore()
        compose.onNodeWithTag("draft").assertDoesNotExist()
        compose.onNodeWithTag("future-capture-toggle").performScrollTo().performClick()
        compose.onNodeWithTag("draft").performScrollTo().assertTextContains("Future plan")
        restoration.emulateSavedInstanceStateRestore()
        compose.onNodeWithTag("draft").performScrollTo().assertTextContains("Future plan")
    }

    @Test
    fun monthlyDayAndIndexLocationOpenTheCorrectDailyDate() {
        start(Journal(entries = listOf(Entry(entryId, today, Log.Daily, Kind.Task, text = "Task"))))
        compose.onNodeWithTag("nav-Monthly").performClick()
        compose.onNodeWithTag("day-3").performScrollTo().performClick()
        compose.onNodeWithTag("date").performScrollTo().assertTextContains("2026-10-03")
        compose.onNodeWithTag("nav-Index").performClick()
        compose.onNodeWithTag("index-$entryId").performScrollTo().performClick()
        compose.onNodeWithTag("date").performScrollTo().assertTextContains("2026-10-04")
    }
}
