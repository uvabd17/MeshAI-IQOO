package ai.meshai.worker.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** Monochrome tokens shared with admin/src/style.css. */
object Mono {
    val Bg = Color(0xFF0B0B0C); val Bg2 = Color(0xFF131315); val Bg3 = Color(0xFF1C1C1F)
    val Fg = Color(0xFFF4F4F5); val Fg2 = Color(0xFFB4B4BA); val Fg3 = Color(0xFF77777F); val Line = Color(0xFF232326)
    val LBg = Color(0xFFFFFFFF); val LBg2 = Color(0xFFF6F6F7); val LBg3 = Color(0xFFECECEE)
    val LFg = Color(0xFF0A0A0A); val LFg2 = Color(0xFF4B4B50); val LFg3 = Color(0xFF8A8A90); val LLine = Color(0xFFE2E2E5)
}

private val Dark = darkColorScheme(
    primary = Mono.Fg, onPrimary = Mono.Bg, secondary = Mono.Fg2, onSecondary = Mono.Bg,
    background = Mono.Bg, onBackground = Mono.Fg, surface = Mono.Bg2, onSurface = Mono.Fg,
    surfaceVariant = Mono.Bg3, onSurfaceVariant = Mono.Fg2, outline = Mono.Line, error = Mono.Fg, onError = Mono.Bg,
)
private val Light = lightColorScheme(
    primary = Mono.LFg, onPrimary = Mono.LBg, secondary = Mono.LFg2, onSecondary = Mono.LBg,
    background = Mono.LBg, onBackground = Mono.LFg, surface = Mono.LBg2, onSurface = Mono.LFg,
    surfaceVariant = Mono.LBg3, onSurfaceVariant = Mono.LFg2, outline = Mono.LLine, error = Mono.LFg, onError = Mono.LBg,
)

@Composable
fun MeshTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) =
    MaterialTheme(colorScheme = if (dark) Dark else Light, content = content)
