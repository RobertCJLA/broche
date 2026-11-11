// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.files

import io.github.robertcjla.broche.ffi.FileToSend
import org.junit.Assert.*
import org.junit.Test

class OutgoingFilesOwnershipTest {
    private val files = listOf(FileToSend.Fd("example.txt", 123, null), FileToSend.Path("other", "/example", null))
    @Test fun `early return closes untransferred descriptors exactly once`() {
        val closed = mutableListOf<Int>()
        val pending = PendingFiles(files) { closed.add(it) }
        pending.close()
        pending.close()
        assertEquals(listOf(123), closed)
    }
    @Test fun `native ownership prevents a second close by Kotlin`() {
        val closed = mutableListOf<Int>()
        PendingFiles(files) { closed.add(it) }.use {
            assertEquals(files, it.take())
        }
        assertTrue(closed.isEmpty())
    }
    @Test fun `exceptions before handoff close every descriptor`() {
        val closed = mutableListOf<Int>()
        try {
            PendingFiles(files) { closed.add(it) }.use { error("node unavailable") }
        } catch (_: IllegalStateException) { }
        assertEquals(listOf(123), closed)
    }
    @Test fun `one failed close does not skip remaining descriptors`() {
        val closed = mutableListOf<Int>()
        PendingFiles(listOf(FileToSend.Fd("a", 1, null), FileToSend.Fd("b", 2, null))) {
            closed.add(it)
            if (it == 1) error("already closed")
        }.close()
        assertEquals(listOf(1, 2), closed)
    }
}
