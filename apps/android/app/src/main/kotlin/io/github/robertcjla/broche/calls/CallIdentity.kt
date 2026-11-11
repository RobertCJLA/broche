// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.calls

internal fun callDisplayName(number: String?, knownName: String?, notificationNames: List<String?>): String? =
    knownName ?: notificationNames.singleOrNull()?.takeIf { number.isNullOrBlank() && it.isNotBlank() }
