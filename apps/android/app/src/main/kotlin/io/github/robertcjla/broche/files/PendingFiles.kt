// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.files

import android.os.ParcelFileDescriptor
import io.github.robertcjla.broche.ffi.FileToSend
import java.io.Closeable

/** Kotlin owns detached descriptors until the native send call takes them. */
internal class PendingFiles(
    private val files: List<FileToSend>,
    private val closeFd: (Int) -> Unit = { ParcelFileDescriptor.adoptFd(it).close() },
) : Closeable {
    private var owned = true

    fun take(): List<FileToSend> {
        check(owned)
        owned = false
        return files
    }

    override fun close() {
        if (!owned) return
        owned = false
        files.filterIsInstance<FileToSend.Fd>().forEach { runCatching { closeFd(it.fd) } }
    }
}
