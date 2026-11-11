// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import io.github.robertcjla.broche.BrocheApplication
import io.github.robertcjla.broche.core.CoreStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Brings the connection back after the phone restarts or Broche is
 * updated, without waiting for the app to be opened. Starting the core starts
 * the connection service when a PC is paired (Core.start). After an update,
 * the new version also says so in a notification.
 */
class StartupReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val updated = when (intent.action) {
            Intent.ACTION_MY_PACKAGE_REPLACED -> true
            Intent.ACTION_BOOT_COMPLETED -> false
            else -> return
        }
        val app = context.applicationContext as BrocheApplication
        val pending = goAsync()
        CoroutineScope(Dispatchers.Default).launch {
            try {
                app.core.start()
                // Kept alive until the core is up, so the service starts inside the
                // window Android allows after these broadcasts.
                withTimeoutOrNull(START_WAIT_MS) { app.core.state.first { it.status !is CoreStatus.Starting } }
            } finally {
                pending.finish()
            }
        }
    }

    private companion object {
        const val START_WAIT_MS = 8_000L
    }
}
