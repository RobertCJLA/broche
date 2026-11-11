// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche

import io.github.robertcjla.broche.ui.Preferences
import io.github.robertcjla.broche.ui.theme.Appearance
import io.github.robertcjla.broche.ui.theme.Tokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AppearanceMigrationTest {

    @Test
    fun `Preferences sanitizeAppearance falls back on unknown legacy theme, mode, and seed values`() {
        val legacy = Preferences.sanitizeAppearance(
            theme = "sepia",
            mode = "neon",
            seed = "cyberpunk",
            dynamicColor = false,
        )
        assertEquals("catppuccin", legacy.theme)
        assertEquals("system", legacy.mode)
        assertEquals(Tokens.DEFAULT_SEED, legacy.seed)
        assertFalse(legacy.dynamicColor)

        val empty = Preferences.sanitizeAppearance(
            Appearance(theme = "", mode = "", seed = "", dynamicColor = true),
        )
        assertEquals("catppuccin", empty.theme)
        assertEquals("system", empty.mode)
        assertEquals(Tokens.DEFAULT_SEED, empty.seed)
        assertTrue(empty.dynamicColor)

        val valid = Preferences.sanitizeAppearance(
            theme = "graphite",
            mode = "dark",
            seed = "ocean",
            dynamicColor = false,
        )
        assertEquals("catppuccin", valid.theme)
        assertEquals("dark", valid.mode)
        assertEquals("ocean", valid.seed)
        assertFalse(valid.dynamicColor)
    }
    @Test
    fun `previous themes migrate to Catppuccin without losing display mode`() {
        for (theme in listOf("graphite", "hive", "glass", "bloom")) {
            for (mode in listOf("system", "light", "dark")) {
                val migrated = Preferences.sanitizeAppearance(theme, mode, "ocean")
                assertEquals("catppuccin", migrated.theme)
                assertEquals(mode, migrated.mode)
            }
        }
    }

}
