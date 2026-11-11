// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.ui.settings

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import io.github.robertcjla.broche.R
import io.github.robertcjla.broche.ffi.DeviceToggle

/** The name shown for each per-device permission (the core's toggle names). */
private fun toggleLabel(name: String): Int? = when (name) {
    "notifications" -> R.string.toggle_notifications
    "messages" -> R.string.toggle_messages
    "calls" -> R.string.toggle_calls
    "contacts" -> R.string.toggle_contacts
    "clipboard" -> R.string.toggle_clipboard
    "files" -> R.string.toggle_files
    "media" -> R.string.toggle_media
    "photos" -> R.string.toggle_photos
    "toggles" -> R.string.toggle_toggles
    "pc_actions" -> R.string.toggle_pc_actions
    "mirroring" -> R.string.toggle_mirroring
    "webcam" -> R.string.toggle_webcam
    "storage" -> R.string.toggle_storage
    "pc_mirror" -> R.string.toggle_pc_mirror
    "remote_input" -> R.string.toggle_remote_input
    "commands" -> R.string.toggle_commands
    "remote_files" -> R.string.toggle_remote_files
    else -> null
}

/**
 * Every per-device permission as a switch, folded away below the profile: most people only
 * need the profile. Starts open when the switches no longer match a profile (`custom`).
 */
@Composable
fun DetailedPermissions(
    toggles: List<DeviceToggle>,
    custom: Boolean,
    onToggle: (name: String, enabled: Boolean) -> Unit,
) {
    var open by rememberSaveable { mutableStateOf(custom) }
    val stateText = stringResource(if (open) R.string.state_expanded else R.string.state_collapsed)
    Column(Modifier.fillMaxWidth()) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .toggleable(value = open, role = Role.Button) { open = it }
                .semantics { stateDescription = stateText },
        ) {
            Text(
                stringResource(R.string.detailed_permissions),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f),
            )
            Text(if (open) "▴" else "▾", style = MaterialTheme.typography.titleMedium)
        }
        AnimatedVisibility(open) {
            Column {
                toggles.forEach { toggle ->
                    val label = toggleLabel(toggle.name) ?: return@forEach
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 48.dp)
                            .toggleable(value = toggle.enabled, role = Role.Switch) { onToggle(toggle.name, it) },
                    ) {
                        Text(stringResource(label), modifier = Modifier.weight(1f).padding(end = 12.dp))
                        Switch(checked = toggle.enabled, onCheckedChange = null)
                    }
                }
            }
        }
    }
}
