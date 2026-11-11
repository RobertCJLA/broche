// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.elevated

import java.io.IOException
import java.net.Socket
import java.security.MessageDigest
import java.util.concurrent.Semaphore
import java.util.concurrent.TimeUnit

internal class HandshakeSlots(limit: Int = 8) {
    private val slots = Semaphore(limit)
    fun tryAcquire(): Boolean = slots.tryAcquire()
    fun release() = slots.release()
}

internal object ElevatedHandshake {
    /** Read only the token, without buffering command bytes past its newline. */
    fun authenticate(client: Socket, token: String, timeoutMillis: Int = 5000): Boolean {
        val previousTimeout = client.soTimeout
        val deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeoutMillis.toLong())
        val bytes = java.io.ByteArrayOutputStream(64)
        return try {
            val input = client.getInputStream()
            var count = 0
            while (true) {
                val remaining = deadline - System.nanoTime()
                if (remaining <= 0) return false
                client.soTimeout = TimeUnit.NANOSECONDS.toMillis(remaining).coerceAtLeast(1).toInt()
                val value = input.read()
                if (value < 0) return false
                if (value == 10) break
                if (++count > 64) return false
                if (value != 13) bytes.write(value)
            }
            MessageDigest.isEqual(bytes.toByteArray(), token.toByteArray(Charsets.US_ASCII))
        } catch (_: IOException) {
            false
        } finally {
            client.soTimeout = previousTimeout
        }
    }
}
