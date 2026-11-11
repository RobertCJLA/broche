// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.view.View
import android.widget.RemoteViews
import android.widget.Toast
import androidx.core.graphics.scale
import io.github.robertcjla.broche.BrocheApplication
import io.github.robertcjla.broche.R
import io.github.robertcjla.broche.clipboard.SendActivity
import io.github.robertcjla.broche.core.CoreState
import io.github.robertcjla.broche.core.CoreStatus
import io.github.robertcjla.broche.core.Device
import io.github.robertcjla.broche.core.WakeState
import io.github.robertcjla.broche.ffi.Link
import io.github.robertcjla.broche.ffi.MediaAction
import io.github.robertcjla.broche.ui.MainActivity

/**
 * Home-screen widget for the main paired PC: how it's connected, what it plays (artwork,
 * previous / play-pause / next), Wake when it's off, and Send files, Copy to PC, PC shortcuts
 * and Control PC. Colors follow the wallpaper on Android 12+ and light/dark mode.
 * `updatePeriodMillis="0"`: never wakes the phone on a timer; [Core] and [PcMedia] refresh it.
 */
class PcWidgetProvider : AppWidgetProvider() {

    override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
        val core = (context.applicationContext as? BrocheApplication)?.core ?: return
        core.start()
        updateWidgets(context, appWidgetManager, appWidgetIds, core.state.value)
    }

    /** Not exported, so other apps can't send these to wake the PC or press its media keys. */
    class ActionReceiver : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val app = context.applicationContext as? BrocheApplication ?: return
            app.core.start()
            val pcId = intent.getStringExtra(EXTRA_PC_ID)?.takeIf { it.isNotBlank() }
                ?: primaryPc(app.core.state.value)?.id
                ?: return
            when (intent.action) {
                ACTION_WAKE_PC -> {
                    app.core.wake(pcId)
                    val name = app.core.state.value.devices.firstOrNull { it.id == pcId }?.name
                        ?.takeIf { it.isNotBlank() } ?: context.getString(R.string.your_pc)
                    Toast.makeText(context, context.getString(R.string.widget_pc_waking_toast, name), Toast.LENGTH_SHORT).show()
                }
                ACTION_MEDIA_PLAY_PAUSE -> app.core.pcMedia.toggleFor(pcId)
                ACTION_MEDIA_NEXT -> app.core.pcMedia.pressedFor(pcId, MediaAction.NEXT)
                ACTION_MEDIA_PREVIOUS -> app.core.pcMedia.pressedFor(pcId, MediaAction.PREVIOUS)
            }
        }
    }

    companion object {
        const val ACTION_WAKE_PC = "io.github.robertcjla.broche.widget.ACTION_WAKE_PC"
        const val ACTION_MEDIA_PLAY_PAUSE = "io.github.robertcjla.broche.widget.ACTION_MEDIA_PLAY_PAUSE"
        const val ACTION_MEDIA_NEXT = "io.github.robertcjla.broche.widget.ACTION_MEDIA_NEXT"
        const val ACTION_MEDIA_PREVIOUS = "io.github.robertcjla.broche.widget.ACTION_MEDIA_PREVIOUS"
        const val EXTRA_PC_ID = "io.github.robertcjla.broche.widget.EXTRA_PC_ID"

        private const val REQ_OPEN_APP = 100
        private const val REQ_SEND_CLIP = 101
        private const val REQ_DECK = 102
        private const val REQ_REMOTE = 103
        private const val REQ_WAKE = 104
        private const val REQ_SEND_FILES = 105
        private const val REQ_MEDIA_PLAY_PAUSE = 106
        private const val REQ_MEDIA_NEXT = 107
        private const val REQ_MEDIA_PREVIOUS = 108

        /** RemoteViews travel through Binder: keep the artwork small. */
        private const val ART_PX = 192

        private fun primaryPc(state: CoreState?): Device? =
            state?.devices?.firstOrNull { it.online } ?: state?.devices?.firstOrNull()

        fun refreshAll(context: Context, state: CoreState? = null) {
            val manager = AppWidgetManager.getInstance(context) ?: return
            val ids = runCatching {
                manager.getAppWidgetIds(ComponentName(context, PcWidgetProvider::class.java))
            }.getOrNull() ?: return
            if (ids.isEmpty()) return
            updateWidgets(
                context,
                manager,
                ids,
                state ?: (context.applicationContext as? BrocheApplication)?.core?.state?.value,
            )
        }

        private fun statusText(context: Context, pc: Device): String {
            val link = pc.link
            return when {
                link is Link.Online && link.relayed -> context.getString(R.string.link_away)
                link is Link.Online && link.usb ->
                    if (link.rttMs >= 1u) context.getString(R.string.link_usb_rtt, link.rttMs.toInt())
                    else context.getString(R.string.link_usb)
                link is Link.Online ->
                    if (link.rttMs >= 1u) context.getString(R.string.link_online_rtt, link.rttMs.toInt())
                    else context.getString(R.string.link_online)
                pc.wakeState == WakeState.Waking -> context.getString(R.string.widget_pc_waking)
                link is Link.Connecting -> context.getString(R.string.link_connecting)
                else -> context.getString(R.string.widget_pc_offline)
            }
        }

        private fun updateWidgets(
            context: Context,
            appWidgetManager: AppWidgetManager,
            appWidgetIds: IntArray,
            state: CoreState?,
        ) {
            if (appWidgetIds.isEmpty()) return
            // While Broche starts, keep what the widget shows instead of flashing "No PC paired".
            if (state == null || (state.devices.isEmpty() && state.status == CoreStatus.Starting)) return
            val pc = primaryPc(state)
            val views = RemoteViews(context.packageName, R.layout.widget_pc)
            views.setOnClickPendingIntent(R.id.widget_root, openApp(context, REQ_OPEN_APP) {})

            if (pc == null) {
                views.setTextViewText(R.id.widget_pc_name, context.getString(R.string.widget_pc_no_pc))
                views.setTextViewText(R.id.widget_pc_status, context.getString(R.string.widget_pc_tap_to_pair))
                views.setViewVisibility(R.id.widget_btn_wake, View.GONE)
                views.setViewVisibility(R.id.widget_media_row, View.GONE)
                views.setViewVisibility(R.id.widget_actions_row, View.GONE)
                appWidgetIds.forEach { appWidgetManager.updateAppWidget(it, views) }
                return
            }

            views.setTextViewText(R.id.widget_pc_name, pc.name.ifBlank { context.getString(R.string.your_pc) })
            views.setTextViewText(R.id.widget_pc_status, statusText(context, pc))

            val waking = pc.wakeState == WakeState.Waking
            views.setViewVisibility(R.id.widget_btn_wake, if (!pc.online && pc.canWake && !waking) View.VISIBLE else View.GONE)
            views.setOnClickPendingIntent(R.id.widget_btn_wake, actionBroadcast(context, REQ_WAKE, ACTION_WAKE_PC, pc.id))

            val media = if (pc.online) {
                (context.applicationContext as? BrocheApplication)?.core?.pcMedia?.currentSummary
                    ?.takeIf { it.pcId == pc.id }
            } else {
                null
            }
            // Always shown, so the widget keeps its layout; with nothing playing, the keys do nothing.
            views.setViewVisibility(R.id.widget_media_row, View.VISIBLE)
            views.setTextViewText(R.id.widget_media_title, media?.title ?: context.getString(R.string.widget_nothing_playing))
            val subtitle = media?.subtitle.orEmpty()
            views.setTextViewText(R.id.widget_media_subtitle, subtitle)
            views.setViewVisibility(R.id.widget_media_subtitle, if (subtitle.isBlank()) View.GONE else View.VISIBLE)
            val art = media?.artwork
            if (art != null) {
                views.setImageViewBitmap(R.id.widget_media_art, small(art))
            } else {
                views.setImageViewResource(R.id.widget_media_art, R.drawable.widget_art_placeholder)
            }
            val playing = media?.playing == true
            views.setImageViewResource(R.id.widget_btn_media_play, if (playing) R.drawable.ic_pause else R.drawable.ic_play)
            views.setContentDescription(
                R.id.widget_btn_media_play,
                context.getString(if (playing) R.string.media_pause else R.string.media_play),
            )
            views.setOnClickPendingIntent(R.id.widget_btn_media_play, actionBroadcast(context, REQ_MEDIA_PLAY_PAUSE, ACTION_MEDIA_PLAY_PAUSE, pc.id))
            views.setOnClickPendingIntent(R.id.widget_btn_media_next, actionBroadcast(context, REQ_MEDIA_NEXT, ACTION_MEDIA_NEXT, pc.id))
            views.setOnClickPendingIntent(R.id.widget_btn_media_previous, actionBroadcast(context, REQ_MEDIA_PREVIOUS, ACTION_MEDIA_PREVIOUS, pc.id))

            views.setViewVisibility(R.id.widget_actions_row, View.VISIBLE)
            views.setOnClickPendingIntent(
                R.id.widget_btn_send_files,
                PendingIntent.getActivity(context, REQ_SEND_FILES, SendActivity.sendFilesIntent(context, pc.id), IMMUTABLE),
            )
            views.setOnClickPendingIntent(
                R.id.widget_btn_send_clip,
                PendingIntent.getActivity(context, REQ_SEND_CLIP, SendActivity.sendClipboardIntent(context, pc.id), IMMUTABLE),
            )
            views.setOnClickPendingIntent(R.id.widget_btn_deck, openApp(context, REQ_DECK) { putExtra("deck_pc", pc.id) })
            views.setOnClickPendingIntent(R.id.widget_btn_remote, openApp(context, REQ_REMOTE) { putExtra("remote_pc", pc.id) })

            appWidgetIds.forEach { appWidgetManager.updateAppWidget(it, views) }
        }

        private const val IMMUTABLE = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE

        private fun small(art: Bitmap): Bitmap {
            val side = maxOf(art.width, art.height)
            if (side <= ART_PX) return art
            val scale = ART_PX.toFloat() / side
            return art.scale((art.width * scale).toInt().coerceAtLeast(1), (art.height * scale).toInt().coerceAtLeast(1))
        }

        private fun openApp(context: Context, requestCode: Int, extras: Intent.() -> Unit): PendingIntent {
            val intent = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
                extras()
            }
            return PendingIntent.getActivity(context, requestCode, intent, IMMUTABLE)
        }

        private fun actionBroadcast(context: Context, requestCode: Int, action: String, pcId: String): PendingIntent {
            val intent = Intent(context, ActionReceiver::class.java).apply {
                this.action = action
                putExtra(EXTRA_PC_ID, pcId)
            }
            return PendingIntent.getBroadcast(context, requestCode, intent, IMMUTABLE)
        }
    }
}
