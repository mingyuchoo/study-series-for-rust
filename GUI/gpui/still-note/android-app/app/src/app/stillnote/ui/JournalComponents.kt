package app.stillnote.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.*
import androidx.compose.ui.unit.dp

@Composable
internal fun Action(
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
                if (nav) this.selected = selected
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
internal fun Panel(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surface, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content,
    )
}
