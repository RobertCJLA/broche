// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.calls

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CallIdentityTest {
    @Test fun `uses the sole call notification when telephony has no identity`() {
        assertEquals("Test caller", callDisplayName(null, null, listOf("Test caller")))
    }

    @Test fun `keeps known identity and does not attach a notification name to another number`() {
        assertEquals("Known caller", callDisplayName(null, "Known caller", listOf("Other caller")))
        assertNull(callDisplayName("5550100", null, listOf("Other caller")))
    }

    @Test fun `does not guess between multiple calls even when one name is missing`() {
        assertNull(callDisplayName(null, null, listOf("First caller", "Second caller")))
        assertNull(callDisplayName(null, null, listOf(null, "Second caller")))
    }

    @Test fun `missing and blank notification identities stay unknown`() {
        assertNull(callDisplayName(null, null, emptyList()))
        assertNull(callDisplayName(null, null, listOf(null)))
        assertNull(callDisplayName(null, null, listOf("  ")))
    }
}
