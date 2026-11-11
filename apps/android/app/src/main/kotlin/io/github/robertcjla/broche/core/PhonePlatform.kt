// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.core

import android.app.ActivityOptions
import android.app.PendingIntent
import android.app.RemoteInput
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import io.github.robertcjla.broche.calls.PhoneCalls
import io.github.robertcjla.broche.camera.ContinuityCameraRequests
import io.github.robertcjla.broche.clipboard.PhoneClipboard
import io.github.robertcjla.broche.contacts.PhoneContacts
import io.github.robertcjla.broche.notifications.NotificationListener
import io.github.robertcjla.broche.media.PhoneMedia
import io.github.robertcjla.broche.photos.RecentPhotos
import io.github.robertcjla.broche.elevated.Elevated
import io.github.robertcjla.broche.mirror.AppWindows
import io.github.robertcjla.broche.mirror.InputService
import io.github.robertcjla.broche.mirror.MirrorRequest
import io.github.robertcjla.broche.mirror.MirrorRequests
import io.github.robertcjla.broche.mirror.MirrorService
import io.github.robertcjla.broche.sms.PhoneSms
import io.github.robertcjla.broche.storage.PhoneStorage
import io.github.robertcjla.broche.toggles.PhoneToggles
import io.github.robertcjla.broche.webcam.WebcamRequest
import io.github.robertcjla.broche.webcam.WebcamRequests
import io.github.robertcjla.broche.webcam.WebcamService
import io.github.robertcjla.broche.ffi.CallCommand
import io.github.robertcjla.broche.ffi.CallLogEntry
import io.github.robertcjla.broche.ffi.Contact
import io.github.robertcjla.broche.ffi.SmsAttachment
import io.github.robertcjla.broche.ffi.SmsMessage
import io.github.robertcjla.broche.ffi.SmsPartData
import io.github.robertcjla.broche.ffi.SmsThread
import io.github.robertcjla.broche.ffi.FileToSend
import io.github.robertcjla.broche.ffi.MediaAction
import io.github.robertcjla.broche.ffi.MirrorInputEvent
import io.github.robertcjla.broche.ffi.MirrorOptions
import io.github.robertcjla.broche.ffi.PhoneApp
import io.github.robertcjla.broche.ffi.PhoneToggleValue
import io.github.robertcjla.broche.ffi.PhotoAlbum
import io.github.robertcjla.broche.ffi.PhotoItem
import io.github.robertcjla.broche.ffi.PhotoThumb
import io.github.robertcjla.broche.ffi.NotificationFailure
import io.github.robertcjla.broche.ffi.Platform
import io.github.robertcjla.broche.ffi.StorageEntry
import io.github.robertcjla.broche.ffi.StorageReadFile
import io.github.robertcjla.broche.ffi.StorageWriteDone
import io.github.robertcjla.broche.ffi.TaskNotify
import io.github.robertcjla.broche.ffi.WebcamOptions

/**
 * What the core asks of the phone: ring it, and act on its notifications
 * for a PC (dismiss, reply, run an action).
 */
internal class PhonePlatform(
    context: Context,
    private val ringer: Ringer,
    private val media: PhoneMedia,
    /** Shows a link a PC sent: (PC's ID, link) → shown. */
    private val onLink: (String, String) -> Boolean,
    /** Shows or updates a watched task from a PC: (PC's ID, task) → shown. */
    private val onTask: (String, TaskNotify) -> Boolean,
    /** The phone's calls (created after this). */
    private val calls: () -> PhoneCalls,
    /** The phone's contacts (created after this). */
    private val contacts: () -> PhoneContacts,
    /** The phone's texts (created after this). */
    private val sms: () -> PhoneSms,
    /** The phone's quick settings (created after this). */
    private val toggles: () -> PhoneToggles,
    /** The phone's shared storage (created after this). */
    private val storage: () -> PhoneStorage,
    /** Asks the user to share the screen with a PC: (PC's ID, request) → asked. */
    private val onMirror: (String, MirrorRequest) -> Boolean,
    /** Asks the user to stream the camera as a webcam to a PC: (PC's ID, request) → asked. */
    private val onWebcam: (String, WebcamRequest) -> Boolean,
    /** Apps in windows of their own on PCs (Elevated). */
    private val appWindows: AppWindows,
    /** Resolves a paired PC's display name by ID. */
    private val nameOf: (String) -> String = { "" },
    /** Notifies when a PC starts or stops typing on this phone without mirroring. */
    private val onKeyboardFromPc: (String, Boolean) -> Unit = { _, _ -> },
) : Platform {
    private val context = context.applicationContext

    override fun startRinging() = ringer.startRinging()

    override fun stopRinging() = ringer.stopRinging()

    override fun setClipboard(text: String): Boolean = PhoneClipboard.write(context, text)

    override fun setClipboardRich(text: String, html: String?, rtf: String?): Boolean =
        PhoneClipboard.write(context, text, html)

    override fun setClipboardImage(mime: String, bytes: ByteArray): Boolean =
        PhoneClipboard.writeImage(context, mime, bytes)

    override fun mediaCommand(player: String, action: MediaAction, position: ULong?) =
        media.command(player, action, position)

    override fun openLink(fromId: String, url: String): Boolean = onLink(fromId, url)

    override fun taskNotify(fromId: String, task: TaskNotify): Boolean = onTask(fromId, task)

    override fun openPhoto(id: String): FileToSend? = RecentPhotos.open(context, id)

    override fun photoAlbums(): List<PhotoAlbum> = RecentPhotos.albums(context)

    override fun photoList(album: String?, before: Long?, beforeId: String, limit: UInt): List<PhotoItem> =
        RecentPhotos.list(context, album, before, beforeId, limit.toInt())

    override fun photoThumbs(ids: List<String>): List<PhotoThumb> = RecentPhotos.thumbs(context, ids)

    override fun callCommand(id: String, command: CallCommand): Boolean = calls().command(id, command)

    override fun callLog(before: Long?, limit: UInt): List<CallLogEntry> =
        calls().callLog(before, limit.toInt())

    override fun callDial(number: String): Boolean = calls().dial(number)

    override fun contacts(query: String?, offset: UInt, limit: UInt): List<Contact> =
        contacts().list(query, offset.toInt(), limit.toInt())

    override fun mirrorRequested(pcId: String, options: MirrorOptions): Boolean =
        if (options.app != null) {
            appWindows.open(pcId, options)
        } else {
            val request = MirrorRequest(
                pcId,
                options.maxSize.toInt(),
                io.github.robertcjla.broche.video.VideoPreferences.limit(context, options.fps.toInt()),
                options.bitrate.toInt(),
                options.audio,
                options.stayAwake,
                options.screenOff,
            )
            if (Elevated.running && MirrorService.startElevated(context, request)) {
                true
            } else {
                onMirror(pcId, request)
            }
        }

    override fun mirrorPowerRequested(pcId: String, stayAwake: Boolean, screenOff: Boolean) {
        MirrorService.setPower(pcId, stayAwake, screenOff)
        appWindows.setPower(pcId, stayAwake, screenOff)
    }

    override fun mirrorStopRequested(pcId: String, session: UInt) {
        if (session != SCREEN) return appWindows.close(pcId, session)
        onKeyboardFromPc(pcId, false)
        MirrorRequests.dismiss(context, pcId)
        MirrorService.stop(pcId)
    }

    override fun mirrorKeyframeRequested(pcId: String, session: UInt) =
        if (session == SCREEN) MirrorService.keyframe(pcId) else appWindows.keyframe(pcId, session)

    override fun mirrorResizeRequested(pcId: String, session: UInt, width: UInt, height: UInt) {
        if (session != SCREEN) appWindows.resize(pcId, session, width.toInt(), height.toInt())
    }

    // Real events when Elevated runs; gestures through the accessibility service otherwise.
    override fun mirrorInput(pcId: String, session: UInt, input: MirrorInputEvent) {
        if (session != SCREEN) return appWindows.input(pcId, session, input)
        if (input is MirrorInputEvent.Key) {
            when (input.key) {
                "keyboard_on" -> {
                    onKeyboardFromPc(pcId, true)
                    return
                }
                "keyboard_off" -> {
                    onKeyboardFromPc(pcId, false)
                    return
                }
            }
        }
        // An injected Recents key opens Overview on the focused display, which while mirroring
        // in advanced mode is the capture's own (seen on Android 16): nothing shows. The
        // accessibility action always opens it on the phone's screen.
        if (input is MirrorInputEvent.Key && input.key == "recents" && InputService.running) {
            InputService.handle(input)
            return
        }
        if (input is MirrorInputEvent.Text && input.text.any { it.code > 127 } && InputService.running) {
            InputService.handle(input)
            return
        }
        if (!Elevated.handle(input)) InputService.handle(input)
    }

    override fun webcamRequested(pcId: String, options: WebcamOptions): Boolean {
        val request = WebcamRequest(
            pcId = pcId,
            width = options.width.toInt(),
            height = options.height.toInt(),
            fps = io.github.robertcjla.broche.video.VideoPreferences.limit(context, options.fps.toInt()),
            bitrate = options.bitrate.toInt(),
            // The camera is chosen on the phone (Webcam screen): keep the last one used
            // there instead of the PC's default.
            camera = io.github.robertcjla.broche.webcam.WebcamService.session.value.camera
                .takeIf { it == "front" || it == "back" } ?: options.camera,
        )
        return onWebcam(pcId, request)
    }

    override fun webcamStopRequested(pcId: String) {
        WebcamRequests.dismiss(context, pcId)
        WebcamService.stopForPc(context, pcId)
    }

    override fun webcamKeyframeRequested(pcId: String) =
        WebcamService.keyframe(pcId)

    override fun cameraCaptureRequested(pcId: String, requestId: String, mode: String): Boolean =
        ContinuityCameraRequests.show(context, pcId, requestId, mode, nameOf(pcId))

    override fun cameraCaptureCancelled(pcId: String, requestId: String) {
        ContinuityCameraRequests.cancelFromPc(context, pcId, requestId)
    }

    override fun phoneApps(): List<PhoneApp> = AppWindows.list(context)

    override fun smsThreads(limit: UInt): List<SmsThread> = sms().threads(limit.toInt())

    override fun smsMessages(thread: String, before: Long?, limit: UInt): List<SmsMessage> =
        sms().messages(thread, before, limit.toInt())

    override fun smsSend(to: List<String>, body: String): Boolean = sms().send(to, body)

    override fun sendSmsWithAttachments(to: List<String>, body: String, attachments: List<SmsAttachment>): Boolean =
        sms().send(to, body, attachments)

    override fun smsPart(id: String): SmsPartData? = sms().part(id)

    override fun setPhoneToggle(id: String, value: PhoneToggleValue): Boolean =
        toggles().set(id, value)

    override fun storageList(path: String): List<StorageEntry> = storage().list(path)

    override fun storageOpenRead(path: String): StorageReadFile = storage().openRead(path)

    override fun storageWrite(path: String, stagedPath: String, modified: Long?): StorageWriteDone =
        storage().write(path, stagedPath, modified)

    override fun storageMkdir(path: String) = storage().mkdir(path)

    override fun storageRename(from: String, to: String) = storage().rename(from, to)

    override fun storageDelete(path: String, confirmed: Boolean) = storage().delete(path, confirmed)

    override fun dismissNotification(key: String) {
        val listener = NotificationListener.instance ?: throw NotificationFailure.Unsupported()
        listener.dismiss(key)
    }

    override fun runNotificationAction(key: String, action: String, reply: String?) {
        val listener = NotificationListener.instance ?: throw NotificationFailure.Unsupported()
        val sbn = listener.find(key) ?: throw NotificationFailure.NotFound()
        val target = action.toIntOrNull()?.let { sbn.notification.actions?.getOrNull(it) }
            ?: throw NotificationFailure.NotFound()
        val intent = target.actionIntent ?: throw NotificationFailure.NotFound()

        // A reply goes to the app the way the notification shade sends it:
        // the text as the result of the action's free-form input.
        val fillIn = Intent()
        if (reply != null) {
            val inputs = target.remoteInputs.orEmpty().filter { it.allowFreeFormInput }
            if (inputs.isEmpty()) throw NotificationFailure.NotFound()
            val results = Bundle().apply { inputs.forEach { putCharSequence(it.resultKey, reply) } }
            RemoteInput.addResultsToIntent(inputs.toTypedArray(), fillIn, results)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                RemoteInput.setResultsSource(fillIn, RemoteInput.SOURCE_FREE_FORM_INPUT)
            }
        }
        try {
            intent.send(context, 0, fillIn, null, null, null, backgroundStartOptions())
        } catch (e: PendingIntent.CanceledException) {
            throw NotificationFailure.NotFound()
        }
    }

    private companion object {
        /** The phone's own screen's mirroring session. */
        const val SCREEN = 0u
    }
}

/** Lets an action open its app's screen, as tapping it on the phone would. */
internal fun backgroundStartOptions(): Bundle? {
    val mode = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.BAKLAVA -> ActivityOptions.MODE_BACKGROUND_ACTIVITY_START_ALLOW_ALWAYS
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE -> {
            @Suppress("DEPRECATION")
            ActivityOptions.MODE_BACKGROUND_ACTIVITY_START_ALLOWED
        }
        else -> return null
    }
    return ActivityOptions.makeBasic().setPendingIntentBackgroundActivityStartMode(mode).toBundle()
}

