package ai.meshai.worker.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/**
 * Neobrutalist tokens, shared in spirit with admin/src/style.css: thick ink borders, hard offset
 * shadows, flat paper surfaces, one loud accent (yellow) plus two soft fills for roles.
 */
data class NbTokens(
    val ink: Color,
    val paper: Color,
    val paper2: Color,
    val muted: Color,
    val accent: Color,
    val host: Color,
    val worker: Color,
    val danger: Color,
)

val LightNb = NbTokens(
    ink = Color(0xFF0A0A0A), paper = Color(0xFFFFFFFF), paper2 = Color(0xFFF3F1EA), muted = Color(0xFF55555C),
    accent = Color(0xFFFFD60A), host = Color(0xFFB8F0C6), worker = Color(0xFFCFC3FF), danger = Color(0xFFFF8A80),
)
val DarkNb = NbTokens(
    ink = Color(0xFFF5F5F5), paper = Color(0xFF121214), paper2 = Color(0xFF1B1B1E), muted = Color(0xFFB5B5BC),
    accent = Color(0xFFFFD60A), host = Color(0xFF2F7A47), worker = Color(0xFF5B4BB5), danger = Color(0xFFB3403A),
)

val LocalNb = staticCompositionLocalOf { LightNb }

private fun scheme(t: NbTokens, dark: Boolean) = if (dark) darkColorScheme(
    primary = t.ink, onPrimary = t.paper, secondary = t.accent, onSecondary = Color(0xFF0A0A0A),
    background = t.paper, onBackground = t.ink, surface = t.paper, onSurface = t.ink,
    surfaceVariant = t.paper2, onSurfaceVariant = t.muted, outline = t.ink, error = t.danger, onError = t.ink,
) else lightColorScheme(
    primary = t.ink, onPrimary = t.paper, secondary = t.accent, onSecondary = Color(0xFF0A0A0A),
    background = t.paper, onBackground = t.ink, surface = t.paper, onSurface = t.ink,
    surfaceVariant = t.paper2, onSurfaceVariant = t.muted, outline = t.ink, error = t.danger, onError = t.ink,
)

@Composable
fun MeshTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val t = if (dark) DarkNb else LightNb
    androidx.compose.runtime.CompositionLocalProvider(LocalNb provides t) {
        MaterialTheme(colorScheme = scheme(t, dark), content = content)
    }
}
