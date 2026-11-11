// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.core

import android.content.Context
import android.content.Intent
import android.provider.Settings

fun openDeveloperOptions(context: Context) {
    val devIntent = Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    if (runCatching { context.startActivity(devIntent) }.isFailure) {
        runCatching {
            context.startActivity(Intent(Settings.ACTION_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        }
    }
}
