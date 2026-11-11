// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.elevated

import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.junit.Assert.*
import org.junit.Test

class InputServerHandshakeTest {
    private val token = "a".repeat(48)

    private fun exchange(send: (Socket) -> Unit): Boolean {
        ServerSocket(0, 8, InetAddress.getLoopbackAddress()).use { server ->
            val executor = Executors.newSingleThreadExecutor()
            try {
                val result = executor.submit<Boolean> {
                    server.accept().use { ElevatedHandshake.authenticate(it, token, 150) }
                }
                Socket(InetAddress.getLoopbackAddress(), server.localPort).use { client ->
                    send(client)
                    return result.get(2, TimeUnit.SECONDS)
                }
            } finally { executor.shutdownNow() }
        }
    }

    @Test fun `valid token leaves command bytes unread and restores timeout`() {
        ServerSocket(0, 8, InetAddress.getLoopbackAddress()).use { server ->
            Socket(InetAddress.getLoopbackAddress(), server.localPort).use { client ->
                client.getOutputStream().write((token + "\nH\n").toByteArray())
                server.accept().use { accepted ->
                    assertTrue(ElevatedHandshake.authenticate(accepted, token, 150))
                    assertEquals(0, accepted.soTimeout)
                    assertEquals('H'.code, accepted.getInputStream().read())
                }
            }
        }
    }
    @Test fun `silent connection expires`() { assertFalse(exchange { }) }
    @Test fun `invalid token is refused`() { assertFalse(exchange { it.getOutputStream().write("wrong\n".toByteArray()) }) }
    @Test fun `oversized token is refused without newline`() { assertFalse(exchange { it.getOutputStream().write(ByteArray(65) { 97 }) }) }
    @Test fun `slow bytes do not extend absolute deadline`() {
        assertFalse(exchange { client ->
            repeat(4) {
                Thread.sleep(60)
                runCatching { client.getOutputStream().write(97) }
            }
        })
    }
    @Test fun `unauthenticated connection slots are bounded and reusable`() {
        val gate = HandshakeSlots(8)
        repeat(8) { assertTrue(gate.tryAcquire()) }
        assertFalse(gate.tryAcquire())
        gate.release()
        assertTrue(gate.tryAcquire())
    }
}
