package app.stillnote.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import app.stillnote.domain.*
import app.stillnote.presentation.JournalStrings

@Composable
internal fun JournalSettingsPanel(
    settings: Settings,
    collections: List<app.stillnote.domain.Collection>,
    collectionName: String,
    busy: Boolean,
    blocked: Boolean,
    strings: JournalStrings,
    onSettings: (Settings) -> Unit,
    onNavigate: (String) -> Unit,
    onCollectionName: (String) -> Unit,
    onCreate: () -> Unit,
) {
    Panel {
        Text(strings.text("언어", "Language"), fontWeight = FontWeight.Bold)
        FlowRow {
            Language.entries.forEach { l ->
                Action(
                    if (l == Language.Korean) "한국어" else "English",
                    "language-$l",
                    l == settings.language,
                ) {
                    onSettings(settings.copy(language = l))
                }
            }
        }
        Text(strings.text("테마", "Theme"), fontWeight = FontWeight.Bold)
        FlowRow {
            ThemeMode.entries.forEach { t ->
                Action(
                    when (t) {
                        ThemeMode.System -> strings.text("시스템", "System")
                        ThemeMode.Light -> strings.text("라이트", "Light")
                        ThemeMode.Dark -> strings.text("다크", "Dark")
                    },
                    "theme-$t",
                    t == settings.theme,
                ) {
                    onSettings(settings.copy(theme = t))
                }
            }
        }
        Text(strings.text("컬렉션", "Collections"), fontWeight = FontWeight.Bold)
        collections.forEach { c ->
            Action(c.name, "collection-${c.id}") { onNavigate(c.id.toString()) }
        }
        OutlinedTextField(
            collectionName,
            onCollectionName,
            enabled = !busy,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { onCreate() }),
            label = { Text(strings.text("컬렉션 이름", "Collection name")) },
            modifier = Modifier.fillMaxWidth().testTag("collection-name"),
        )
        Action(
            strings.text("컬렉션 만들기", "Create collection"),
            "collection-create",
            enabled = !blocked && !busy,
        ) {
            onCreate()
        }
    }
}
