// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.clipboard

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Test

class ShareEntryPointsTest {
    @Test fun `internal clipboard entry point is not exported`() {
        val factory = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }
        val doc = factory.newDocumentBuilder().parse(File("src/main/AndroidManifest.xml"))
        val activities = doc.getElementsByTagName("activity")
        val ns = "http://schemas.android.com/apk/res/android"
        val exported = (0 until activities.length).map { activities.item(it) as org.w3c.dom.Element }
            .associate { it.getAttributeNS(ns, "name") to it.getAttributeNS(ns, "exported") }
        assertEquals("false", exported[".clipboard.InternalSendActivity"])
        assertEquals("true", exported[".clipboard.SendActivity"])
    }
}
