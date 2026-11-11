// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.elevated

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class ElevatedGrantsTest {
    private fun source(name: String) = File("src/main/kotlin/io/github/robertcjla/broche/elevated/$name").readText()

    /** The body of `private fun <name>(` in [text], up to the function's closing brace. */
    private fun body(text: String, name: String): String {
        val start = text.indexOf("private fun $name(")
        check(start >= 0) { "$name not found" }
        return text.substring(start, text.indexOf("\n    }\n", start))
    }

    @Test fun `removed banking and radio controls are not available to helper commands`() {
        val source = source("InputServer.kt")
        for (command in listOf("GET_USAGE_STATS", "svc wifi", "svc bluetooth", "hotspot on", "start-softap")) {
            assertFalse("residual command: $command", source.contains(command))
        }
    }

    @Test fun `starting advanced mode never grants call control or sensitive notifications`() {
        val startup = body(source("InputServer.kt"), "grantAppOps") + body(source("Elevated.kt"), "grants")
        for (word in listOf("MANAGE_ONGOING_CALLS", "RECEIVE_SENSITIVE_NOTIFICATIONS", "OP_NAME", "OP_SENSITIVE", "USER_APP_OPS", "appOp")) {
            assertFalse("advanced mode's start grants $word", startup.contains(word))
        }
    }

    @Test fun `only the settings switches turn those app-ops on`() {
        val callers = File("src/main/kotlin").walkTopDown()
            .filter { it.extension == "kt" && it.readText().contains("Elevated.setAppOp(") }
            .map { it.name }
            .toList()
        assertEquals(listOf("SettingsScreen.kt"), callers)
    }
}
