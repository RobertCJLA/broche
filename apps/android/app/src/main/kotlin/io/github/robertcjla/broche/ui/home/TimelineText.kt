// SPDX-License-Identifier: MPL-2.0
package io.github.robertcjla.broche.ui.home

import io.github.robertcjla.broche.R

/**
 * The core writes some history titles in English ("Copied text: …", "Files", "a.txt (+2 more)").
 * Shows those in the app's language; anything else (file names, page titles) stays as it is.
 * [str] is `Context.getString`.
 */
internal fun localizedTimelineTitle(title: String, str: (Int, Array<out Any>) -> String): String {
    fun s(id: Int, vararg args: Any) = str(id, args)
    when (title) {
        "Files" -> return s(R.string.timeline_t_files)
        "Copied text", "Text clip" -> return s(R.string.timeline_t_text_clip)
        "Image clip" -> return s(R.string.timeline_t_image_clip)
        "Map location" -> return s(R.string.timeline_t_map_location)
        "Handoff document" -> return s(R.string.timeline_t_handoff_document)
        "Phone webcam" -> return s(R.string.timeline_t_phone_webcam)
    }
    title.removePrefix("Copied text: ").takeIf { it != title }?.let { return s(R.string.timeline_t_copied_text, it) }
    title.removePrefix("Copied file ").takeIf { it != title }?.let { return s(R.string.timeline_t_copied_file, it) }
    title.removePrefix("Link · ").takeIf { it != title }?.let { return s(R.string.timeline_t_link, it) }
    title.removeSuffix(" on a map").takeIf { it != title }?.let { return s(R.string.timeline_t_on_map, it) }
    Regex("""^(.*) \(\+(\d+) more\)$""").find(title)?.let {
        // A file name could carry a huge number: keep the title as it is then.
        val more = it.groupValues[2].toIntOrNull() ?: return title
        return s(R.string.timeline_t_more, it.groupValues[1], more)
    }
    return title
}
