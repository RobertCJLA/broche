// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.clipboard

/** External shares must not borrow access to this app's private file provider. */
internal fun isAllowedSharedUri(scheme: String?, authority: String?, packageName: String): Boolean {
    if (scheme != "content" || authority.isNullOrBlank()) return false
    val provider = authority.substringAfterLast('@')
    return !provider.equals("$packageName.files", ignoreCase = true)
}
