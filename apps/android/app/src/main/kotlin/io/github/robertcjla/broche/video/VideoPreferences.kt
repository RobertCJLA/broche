// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.video
import android.content.Context
import androidx.core.content.edit
object VideoPreferences {
    fun experimental(context: Context) = context.getSharedPreferences("video", 0).getBoolean("experimental60", false)
    fun setExperimental(context: Context, enabled: Boolean) {
        context.getSharedPreferences("video", 0).edit().putBoolean("experimental60", enabled).apply()
    }
    fun preferred(context: Context) = if (experimental(context)) 60 else 30
    fun limit(context: Context, requested: Int) = cap(requested, experimental(context))
    fun cap(requested: Int, experimental: Boolean) = requested.coerceIn(1, if (experimental) 60 else 30)

    fun webcam1080pExperimental(context: Context) = context.getSharedPreferences("video", 0).getBoolean("webcam1080p", false)
    fun setWebcam1080pExperimental(context: Context, enabled: Boolean) {
        context.getSharedPreferences("video", 0).edit().putBoolean("webcam1080p", enabled).apply()
    }
    fun webcamResolutionHeight(context: Context): Int = if (webcam1080pExperimental(context)) 1080 else 720

    /** The one video quality, kept the same as the PC's: high = 1080p and 60 fps. */
    fun high(context: Context) = experimental(context)

    /** When the quality was chosen (Unix ms): the newer choice wins on both devices. */
    fun chosenAt(context: Context) = context.getSharedPreferences("video", 0).getLong("chosen_at", 0L)

    fun setHigh(context: Context, high: Boolean, at: Long) {
        context.getSharedPreferences("video", 0).edit {
            putBoolean("experimental60", high)
            putBoolean("webcam1080p", high)
            putLong("chosen_at", at)
        }
    }
}
