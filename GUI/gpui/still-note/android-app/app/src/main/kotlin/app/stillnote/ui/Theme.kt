package app.stillnote.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.*
import app.stillnote.R
import app.stillnote.domain.ThemeMode

val Pretendard =
    FontFamily(
        Font(R.font.pretendard_regular, FontWeight.Normal),
        Font(R.font.pretendard_medium, FontWeight.Medium),
        Font(R.font.pretendard_semibold, FontWeight.SemiBold),
        Font(R.font.pretendard_bold, FontWeight.Bold),
    )
private val dark =
    darkColorScheme(
        primary = Color(0xfffaff69),
        onPrimary = Color(0xff0a0a0a),
        background = Color(0xff0a0a0a),
        onBackground = Color.White,
        surface = Color(0xff1a1a1a),
        surfaceContainer = Color(0xff1a1a1a),
        surfaceContainerHigh = Color(0xff1a1a1a),
        surfaceContainerHighest = Color(0xff242424),
        surfaceContainerLow = Color(0xff121212),
        surfaceContainerLowest = Color(0xff0a0a0a),
        surfaceTint = Color.Transparent,
        secondary = Color(0xffcccccc),
        onSecondary = Color(0xff0a0a0a),
        secondaryContainer = Color(0xff242424),
        onSecondaryContainer = Color(0xffcccccc),
        onSurface = Color(0xffcccccc),
        surfaceVariant = Color(0xff242424),
        onSurfaceVariant = Color(0xff888888),
        outline = Color(0xff3a3a3a),
    )
private val light =
    lightColorScheme(
        primary = Color(0xff555a00),
        onPrimary = Color.White,
        background = Color(0xfff7f7f5),
        onBackground = Color(0xff191919),
        surface = Color.White,
        surfaceContainer = Color.White,
        surfaceContainerHigh = Color.White,
        surfaceContainerHighest = Color(0xffe1e1db),
        surfaceContainerLow = Color(0xffebebe7),
        surfaceContainerLowest = Color(0xfff7f7f5),
        surfaceTint = Color.Transparent,
        secondary = Color(0xff333333),
        onSecondary = Color.White,
        secondaryContainer = Color(0xffe1e1db),
        onSecondaryContainer = Color(0xff333333),
        onSurface = Color(0xff333333),
        surfaceVariant = Color(0xffe1e1db),
        onSurfaceVariant = Color(0xff595959),
        outline = Color(0xff898980),
    )

@Composable
fun StillnoteTheme(mode: ThemeMode, content: @Composable () -> Unit) {
    val d =
        when (mode) {
            ThemeMode.System -> isSystemInDarkTheme()
            ThemeMode.Dark -> true
            ThemeMode.Light -> false
        }
    val t = Typography()
    MaterialTheme(
        colorScheme = if (d) dark else light,
        typography =
            Typography(
                displayLarge = t.displayLarge.copy(fontFamily = Pretendard),
                displayMedium = t.displayMedium.copy(fontFamily = Pretendard),
                displaySmall = t.displaySmall.copy(fontFamily = Pretendard),
                headlineSmall = t.headlineSmall.copy(fontFamily = Pretendard),
                titleSmall = t.titleSmall.copy(fontFamily = Pretendard),
                headlineLarge =
                    t.headlineLarge.copy(fontFamily = Pretendard, fontWeight = FontWeight.Bold),
                headlineMedium =
                    t.headlineMedium.copy(fontFamily = Pretendard, fontWeight = FontWeight.Bold),
                titleLarge =
                    t.titleLarge.copy(fontFamily = Pretendard, fontWeight = FontWeight.Bold),
                titleMedium = t.titleMedium.copy(fontFamily = Pretendard),
                bodyLarge = t.bodyLarge.copy(fontFamily = Pretendard),
                bodyMedium = t.bodyMedium.copy(fontFamily = Pretendard),
                bodySmall = t.bodySmall.copy(fontFamily = Pretendard),
                labelLarge =
                    t.labelLarge.copy(fontFamily = Pretendard, fontWeight = FontWeight.SemiBold),
                labelMedium = t.labelMedium.copy(fontFamily = Pretendard),
                labelSmall = t.labelSmall.copy(fontFamily = Pretendard),
            ),
        content = content,
    )
}
