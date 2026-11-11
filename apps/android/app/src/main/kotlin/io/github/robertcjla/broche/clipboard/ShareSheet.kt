// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.clipboard

import android.content.Context
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.util.Size
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.robertcjla.broche.R
import io.github.robertcjla.broche.core.Device
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** What a share shows before it goes: the picture, the files, or the text. */
internal sealed interface SharePreview {
    data class Files(val uris: List<Uri>) : SharePreview
    data class Text(val text: String) : SharePreview
}

/**
 * The confirmation every share from another app shows (in Broche's theme): what goes, to
 * which PC, and Send or Copy. Other apps can open the share entry without a share gesture,
 * so nothing is sent until the user confirms.
 */
@Composable
internal fun ShareSheet(
    preview: SharePreview,
    toClipboard: Boolean,
    pcs: List<Device>,
    initialPcId: String?,
    onConfirm: (pcId: String?) -> Unit,
    onCancel: () -> Unit,
) {
    var picked by remember { mutableStateOf(initialPcId ?: (pcs.firstOrNull { it.online } ?: pcs.firstOrNull())?.id) }
    AlertDialog(
        onDismissRequest = onCancel,
        icon = {
            Icon(painterResource(if (toClipboard) R.drawable.ic_clipboard else R.drawable.ic_devices), contentDescription = null)
        },
        title = { Text(stringResource(if (toClipboard) R.string.share_copy_title else R.string.share_send_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                PreviewOf(preview)
                when {
                    pcs.size > 1 -> PcChoice(pcs, picked) { picked = it }
                    pcs.size == 1 -> Text(
                        stringResource(R.string.share_to_pc, pcs[0].name) + " · " + stringResource(
                            if (pcs[0].online) R.string.share_pc_online else R.string.share_pc_offline,
                        ),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
                Text(
                    stringResource(if (toClipboard) R.string.share_copy_hint else R.string.share_confirm_body),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = {
            Button(onClick = { onConfirm(picked) }, enabled = pcs.isNotEmpty()) {
                Text(stringResource(if (toClipboard) R.string.share_copy_action else R.string.share_send_action))
            }
        },
        dismissButton = { TextButton(onClick = onCancel) { Text(stringResource(R.string.action_cancel)) } },
    )
}

@Composable
private fun PreviewOf(preview: SharePreview) {
    val context = LocalContext.current
    when (preview) {
        is SharePreview.Text -> Surface(
            color = MaterialTheme.colorScheme.surfaceContainerHighest,
            shape = RoundedCornerShape(16.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                preview.text,
                maxLines = 4,
                overflow = TextOverflow.Ellipsis,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(12.dp),
            )
        }
        is SharePreview.Files -> {
            val first = preview.uris.first()
            val thumb by produceState<ImageBitmap?>(null, first) { value = withContext(Dispatchers.IO) { thumbnail(context, first) } }
            val name by produceState<String?>(null, first) { value = withContext(Dispatchers.IO) { displayName(context, first) } }
            thumb?.let {
                Image(
                    it,
                    contentDescription = name,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxWidth().heightIn(max = 220.dp).clip(RoundedCornerShape(16.dp)),
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(painterResource(R.drawable.ic_folder_open), contentDescription = null, modifier = Modifier.size(20.dp))
                Text(
                    if (preview.uris.size == 1) name.orEmpty() else pluralStringResource(R.plurals.share_files, preview.uris.size, preview.uris.size),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
    }
}

@Composable
private fun PcChoice(pcs: List<Device>, picked: String?, onPick: (String) -> Unit) {
    Column(Modifier.selectableGroup()) {
        Text(stringResource(R.string.share_pick_pc), style = MaterialTheme.typography.titleSmall)
        pcs.forEach { pc ->
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = 48.dp)
                    .selectable(selected = pc.id == picked, role = Role.RadioButton) { onPick(pc.id) },
            ) {
                RadioButton(selected = pc.id == picked, onClick = null)
                Column(Modifier.padding(start = 12.dp)) {
                    Text(pc.name, style = MaterialTheme.typography.bodyLarge)
                    Text(
                        stringResource(if (pc.online) R.string.share_pc_online else R.string.share_pc_offline),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

/** A small picture of a shared image, or null when it isn't one. */
private fun thumbnail(context: Context, uri: Uri): ImageBitmap? = runCatching {
    val resolver = context.contentResolver
    if (resolver.getType(uri)?.startsWith("image/") != true) return null
    val px = 480
    val bitmap = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        resolver.loadThumbnail(uri, Size(px, px), null)
    } else {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, bounds) }
        var sample = 1
        while (bounds.outWidth / (sample * 2) >= px && bounds.outHeight / (sample * 2) >= px) sample *= 2
        resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, BitmapFactory.Options().apply { inSampleSize = sample }) }
    }
    bitmap?.asImageBitmap()
}.getOrNull()

private fun displayName(context: Context, uri: Uri): String? = runCatching {
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
        if (c.moveToFirst()) c.getString(0) else null
    }
}.getOrNull() ?: uri.lastPathSegment
