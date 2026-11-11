// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.clipboard

import org.junit.Assert.*
import org.junit.Test

class ShareUriPolicyTest {
    private val pkg = "io.github.robertcjla.broche"

    @Test fun `external shares accept content from other providers only`() {
        assertTrue(isAllowedSharedUri("content", "com.android.providers.downloads.documents", pkg))
        assertFalse(isAllowedSharedUri("file", null, pkg))
        assertFalse(isAllowedSharedUri("https", "example.org", pkg))
        assertFalse(isAllowedSharedUri("content", "$pkg.files", pkg))
        assertFalse(isAllowedSharedUri("content", "0@$pkg.files", pkg))
        assertFalse(isAllowedSharedUri("content", "", pkg))
        assertFalse(isAllowedSharedUri(null, null, pkg))
    }
}
