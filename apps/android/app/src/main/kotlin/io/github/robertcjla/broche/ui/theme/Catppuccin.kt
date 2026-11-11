// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.luminance

/**
 * Material 3 colour roles coloured with Catppuccin or Tokyo Night. The same mapping and
 * the same contrast rules as the Linux app (`apps/linux/src/theme.rs`):
 * text reaches 4.5:1 and outlines 3:1 on every surface, so cards never lose their edges.
 */
private class Flavor(
    val accent: Long, val red: Long, val green: Long, val blue: Long,
    val text: Long, val subtext: Long, val overlay1: Long, val overlay0: Long,
    val surface2: Long, val surface1: Long, val surface0: Long,
    val base: Long, val mantle: Long, val crust: Long,
)

// https://catppuccin.com/palette
private val MOCHA = Flavor(0xFFCBA6F7, 0xFFF38BA8, 0xFFA6E3A1, 0xFF89B4FA, 0xFFCDD6F4, 0xFFA6ADC8, 0xFF7F849C,
    0xFF6C7086, 0xFF585B70, 0xFF45475A, 0xFF313244, 0xFF1E1E2E, 0xFF181825, 0xFF11111B)
private val MACCHIATO = Flavor(0xFFC6A0F6, 0xFFED8796, 0xFFA6DA95, 0xFF8AADF4, 0xFFCAD3F5, 0xFFA5ADCB, 0xFF8087A2,
    0xFF6E738D, 0xFF5B6078, 0xFF494D64, 0xFF363A4F, 0xFF24273A, 0xFF1E2030, 0xFF181926)
private val FRAPPE = Flavor(0xFFCA9EE6, 0xFFE78284, 0xFFA6D189, 0xFF8CAAEE, 0xFFC6D0F5, 0xFFA5ADCE, 0xFF838BA7,
    0xFF737994, 0xFF626880, 0xFF51576D, 0xFF414559, 0xFF303446, 0xFF292C3C, 0xFF232634)
private val LATTE = Flavor(0xFF8839EF, 0xFFD20F39, 0xFF40A02B, 0xFF1E66F5, 0xFF4C4F69, 0xFF6C6F85, 0xFF8C8FA1,
    0xFF9CA0B0, 0xFFACB0BE, 0xFFBCC0CC, 0xFFCCD0DA, 0xFFEFF1F5, 0xFFE6E9EF, 0xFFDCE0E8)
// https://github.com/folke/tokyonight.nvim
private val TOKYO_NIGHT = Flavor(0xFF7AA2F7, 0xFFF7768E, 0xFF9ECE6A, 0xFF7DCFFF, 0xFFC0CAF5, 0xFFA9B1D6, 0xFF737AA2,
    0xFF565F89, 0xFF545C7E, 0xFF414868, 0xFF292E42, 0xFF1A1B26, 0xFF16161E, 0xFF0F0F14)
private val TOKYO_STORM = Flavor(0xFF7AA2F7, 0xFFF7768E, 0xFF9ECE6A, 0xFF7DCFFF, 0xFFC0CAF5, 0xFFA9B1D6, 0xFF737AA2,
    0xFF565F89, 0xFF545C7E, 0xFF414868, 0xFF292E42, 0xFF24283B, 0xFF1F2335, 0xFF1B1E2D)
private val TOKYO_MOON = Flavor(0xFF82AAFF, 0xFFFF757F, 0xFFC3E88D, 0xFF86E1FC, 0xFFC8D3F5, 0xFFA9B8E8, 0xFF737AA2,
    0xFF636DA6, 0xFF545C7E, 0xFF444A73, 0xFF2F334D, 0xFF222436, 0xFF1E2030, 0xFF191A2A)
private val TOKYO_DAY = Flavor(0xFF2E7DE9, 0xFFC64343, 0xFF587539, 0xFF007197, 0xFF3760BF, 0xFF4F5E9C, 0xFF68709A,
    0xFF848CB5, 0xFFA1A6C5, 0xFFB7C1E3, 0xFFC4C8DA, 0xFFE1E2E7, 0xFFD0D5E3, 0xFFC7CBDB)

/** The theme ids offered in Settings, with their names. "system" is the wallpaper's colours. */
val SCHEME_CHOICES = listOf(
    "catppuccin" to "Catppuccin Mocha / Latte",
    "macchiato" to "Catppuccin Macchiato / Latte",
    "frappe" to "Catppuccin Frappé / Latte",
    "tokyo" to "Tokyo Night / Day",
    "tokyo-storm" to "Tokyo Night Storm / Day",
    "tokyo-moon" to "Tokyo Night Moon / Day",
)

private fun c(v: Long) = Color(v)

/** WCAG contrast ratio of two opaque colours. */
internal fun contrast(a: Color, b: Color): Float {
    val x = a.luminance()
    val y = b.luminance()
    return (maxOf(x, y) + 0.05f) / (minOf(x, y) + 0.05f)
}

/** [fg] pushed towards [towards] just enough to reach [min] contrast on all [backgrounds]. */
private fun ensure(fg: Color, backgrounds: List<Color>, min: Float, towards: Color): Color =
    (0..40).map { lerp(fg, towards, it / 40f) }.firstOrNull { c -> backgrounds.all { contrast(c, it) >= min } } ?: towards

/** Raises text and outlines of any scheme (also the wallpaper's) to WCAG AA. */
internal fun ColorScheme.accessible(dark: Boolean): ColorScheme {
    val towards = if (dark) Color.White else Color.Black
    val surfaces = listOf(surface, surfaceContainerLow, surfaceContainer, surfaceContainerHigh)
    var p = if (dark) primary else ensure(primary, listOf(Color.White), 4.5f, Color.Black)
    p = ensure(p, surfaces, 3f, towards)
    return copy(
        primary = p,
        onPrimary = ensure(onPrimary, listOf(p), 4.5f, if (dark) Color.Black else Color.White),
        onSurface = ensure(onSurface, surfaces, 4.5f, towards),
        onBackground = ensure(onBackground, listOf(background), 4.5f, towards),
        onSurfaceVariant = ensure(onSurfaceVariant, surfaces, 4.5f, towards),
        outline = ensure(outline, surfaces, 3f, towards),
        onPrimaryContainer = ensure(onPrimaryContainer, listOf(primaryContainer), 4.5f, towards),
        onSecondaryContainer = ensure(onSecondaryContainer, listOf(secondaryContainer), 4.5f, towards),
        onTertiaryContainer = ensure(onTertiaryContainer, listOf(tertiaryContainer), 4.5f, towards),
        onErrorContainer = ensure(onErrorContainer, listOf(errorContainer), 4.5f, towards),
        error = ensure(error, surfaces, 3f, towards),
    )
}

private fun roles(f: Flavor, dark: Boolean): ColorScheme {
    val accent = c(f.accent)
    val base = c(f.base)
    return if (dark) {
        darkColorScheme().copy(
            primary = accent, onPrimary = c(f.crust),
            primaryContainer = lerp(base, accent, 0.32f), onPrimaryContainer = lerp(accent, Color.White, 0.55f),
            inversePrimary = lerp(accent, Color.Black, 0.35f),
            secondary = c(f.blue), onSecondary = c(f.crust),
            secondaryContainer = lerp(c(f.surface0), accent, 0.12f), onSecondaryContainer = c(f.text),
            tertiary = c(f.green), onTertiary = c(f.crust),
            tertiaryContainer = lerp(base, c(f.green), 0.25f), onTertiaryContainer = lerp(c(f.green), Color.White, 0.5f),
            background = base, onBackground = c(f.text),
            surface = base, onSurface = c(f.text),
            surfaceVariant = c(f.surface0), onSurfaceVariant = c(f.subtext),
            surfaceTint = accent,
            surfaceContainerLowest = c(f.crust), surfaceContainerLow = c(f.mantle),
            surfaceContainer = lerp(base, c(f.surface0), 0.45f),
            surfaceContainerHigh = c(f.surface0), surfaceContainerHighest = c(f.surface1),
            surfaceBright = c(f.surface1), surfaceDim = c(f.mantle),
            outline = c(f.overlay1), outlineVariant = c(f.surface2),
            inverseSurface = c(f.text), inverseOnSurface = base,
            error = c(f.red), onError = c(f.crust),
            errorContainer = lerp(base, c(f.red), 0.28f), onErrorContainer = lerp(c(f.red), Color.White, 0.5f),
            scrim = Color.Black,
        )
    } else {
        lightColorScheme().copy(
            primary = accent, onPrimary = Color.White,
            primaryContainer = lerp(base, accent, 0.18f), onPrimaryContainer = lerp(accent, Color.Black, 0.45f),
            inversePrimary = lerp(accent, Color.White, 0.4f),
            secondary = c(f.blue), onSecondary = Color.White,
            secondaryContainer = lerp(c(f.mantle), accent, 0.10f), onSecondaryContainer = c(f.text),
            tertiary = c(f.green), onTertiary = Color.White,
            tertiaryContainer = lerp(base, c(f.green), 0.18f), onTertiaryContainer = lerp(c(f.green), Color.Black, 0.45f),
            background = base, onBackground = c(f.text),
            surface = base, onSurface = c(f.text),
            surfaceVariant = c(f.surface0), onSurfaceVariant = c(f.subtext),
            surfaceTint = accent,
            surfaceContainerLowest = Color.White, surfaceContainerLow = lerp(base, Color.White, 0.45f),
            surfaceContainer = c(f.mantle), surfaceContainerHigh = c(f.crust), surfaceContainerHighest = c(f.surface0),
            surfaceBright = Color.White, surfaceDim = c(f.crust),
            outline = lerp(c(f.overlay0), c(f.text), 0.35f), outlineVariant = c(f.surface1),
            inverseSurface = c(f.text), inverseOnSurface = base,
            error = c(f.red), onError = Color.White,
            errorContainer = lerp(base, c(f.red), 0.16f), onErrorContainer = lerp(c(f.red), Color.Black, 0.4f),
            scrim = Color.Black,
        )
    }
}

/** The scheme for a theme id (see [SCHEME_CHOICES]) in light or dark mode. */
internal fun catppuccinScheme(dark: Boolean, flavor: String = "catppuccin"): ColorScheme {
    val tokyo = flavor.startsWith("tokyo")
    val f = when {
        !dark -> if (tokyo) TOKYO_DAY else LATTE
        flavor == "macchiato" -> MACCHIATO
        flavor == "frappe" -> FRAPPE
        flavor == "tokyo" -> TOKYO_NIGHT
        flavor == "tokyo-storm" -> TOKYO_STORM
        flavor == "tokyo-moon" -> TOKYO_MOON
        else -> MOCHA
    }
    return roles(f, dark).accessible(dark)
}
