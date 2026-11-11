// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.files

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.*
import org.junit.Test

class ReceivedFilesPathsTest {
    @Test fun `received files are shared from Broche's folder only`() {
        val doc = DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(File("src/main/res/xml/file_paths.xml"))
        val paths = doc.getElementsByTagName("external-files-path")
        val directories = (0 until paths.length).associate {
            val attrs = paths.item(it).attributes
            attrs.getNamedItem("name").nodeValue to attrs.getNamedItem("path").nodeValue
        }
        assertNull(directories["received"])
        assertEquals("Download/Broche/", directories["received_broche"])
    }

    @Test fun `file provider includes Broche without exposing whole external storage`() {
        val doc = DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(File("src/main/res/xml/file_paths.xml"))
        val paths = doc.getElementsByTagName("external-files-path")
        val allowed = (0 until paths.length).map { paths.item(it).attributes.getNamedItem("path").nodeValue }
        assertTrue(allowed.contains("Download/Broche/"))
        assertFalse(allowed.any { it.isEmpty() || it == "." || it == "/" })
    }
}
