// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche

import android.app.Application
import io.github.robertcjla.broche.core.Core
import io.github.robertcjla.broche.elevated.Elevated
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/** Starts the core with the process and keeps it for the app's lifetime. */
class BrocheApplication : Application() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    val core: Core by lazy { Core(this, scope) }

    override fun onCreate() {
        super.onCreate()
        Elevated.init(this)
        // What the phone offers PCs follows Elevated starting and stopping.
        Elevated.onChange = { core.refreshNotificationAccess() }
        Elevated.onClipboardChanged = { text, html -> core.onElevatedClipboardChanged(text, html) }
        android.view.Choreographer.getInstance().postFrameCallback {
            scope.launch { core.start() }
        }
        scope.launch {
            kotlinx.coroutines.delay(120)
            core.start()
        }
    }
}
