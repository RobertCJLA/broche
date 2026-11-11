// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.ui.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import io.github.robertcjla.broche.R
import io.github.robertcjla.broche.ffi.PermissionProfile

/**
 * Picks what a PC may do on this phone, as one of the core's permission profiles.
 * `selected` is null when the permissions were changed one by one ("custom").
 */
@Composable
fun PermissionProfilePicker(
    selected: PermissionProfile?,
    onSelect: (PermissionProfile) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier.selectableGroup()) {
        if (selected == null) {
            Text(
                stringResource(R.string.profile_custom),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        PermissionProfile.entries.forEach { profile ->
            val (title, body) = when (profile) {
                PermissionProfile.NOTIFICATIONS_ONLY -> R.string.profile_notifications_only to R.string.profile_notifications_only_body
                PermissionProfile.BASIC -> R.string.profile_basic to R.string.profile_basic_body
                PermissionProfile.MULTIMEDIA -> R.string.profile_multimedia to R.string.profile_multimedia_body
                PermissionProfile.FULL -> R.string.profile_full to R.string.profile_full_body
            }
            Row(
                verticalAlignment = Alignment.Top,
                modifier = Modifier
                    .fillMaxWidth()
                    .selectable(selected = profile == selected, role = Role.RadioButton, onClick = { onSelect(profile) })
                    .padding(vertical = 8.dp),
            ) {
                RadioButton(selected = profile == selected, onClick = null)
                Column(Modifier.padding(start = 12.dp)) {
                    Text(stringResource(title), style = MaterialTheme.typography.bodyLarge)
                    Text(
                        stringResource(body),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}
