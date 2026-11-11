// SPDX-License-Identifier: MPL-2.0
package io.github.robertcjla.broche.ui

import io.github.robertcjla.broche.R
import io.github.robertcjla.broche.ui.home.localizedTimelineTitle
import org.junit.Assert.assertEquals
import org.junit.Test

class TimelineTextTest {
    // Stands in for getString: the resource id and its arguments.
    private fun tr(title: String) = localizedTimelineTitle(title) { id, args ->
        "$id:" + args.joinToString(",")
    }

    @Test
    fun coreEnglishTitlesAreTranslated() {
        assertEquals("${R.string.timeline_t_copied_text}:hola mundo…", tr("Copied text: hola mundo…"))
        assertEquals("${R.string.timeline_t_more}:a.txt,2", tr("a.txt (+2 more)"))
        assertEquals("${R.string.timeline_t_files}:", tr("Files"))
        assertEquals("${R.string.timeline_t_link}:example.com", tr("Link · example.com"))
    }

    @Test
    fun otherTitlesStayAsTheyAre() {
        assertEquals("informe.pdf", tr("informe.pdf"))
        assertEquals("Short note", tr("Short note"))
        assertEquals("x (+99999999999 more)", tr("x (+99999999999 more)"))
    }
}
