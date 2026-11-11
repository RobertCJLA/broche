// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.ui.home

import android.content.Intent
import android.net.Uri
import android.text.format.Formatter
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.robertcjla.broche.R
import io.github.robertcjla.broche.core.Device
import io.github.robertcjla.broche.files.ReceivedFiles
import io.github.robertcjla.broche.ui.theme.LocalReducedMotion
import io.github.robertcjla.broche.ui.theme.ScreenHeader
import io.github.robertcjla.broche.ffi.RemoteStorageEntry
import kotlinx.coroutines.launch

/**
 * The PC's shared folder as its own full screen page: browsing, quick folders,
 * hidden files and downloads. It used to be a Dialog, whose separate window kept
 * a near-black background behind the status bar; as a route it inherits the
 * app theme like every other screen.
 */
@Composable
fun PcFilesScreen(
    device: Device,
    onListFiles: (path: String, onResult: (List<RemoteStorageEntry>, String?) -> Unit) -> Unit,
    onDownloadFile: (remotePath: String, fileName: String, onSaved: (Uri?) -> Unit) -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val resources = LocalResources.current
    val scope = rememberCoroutineScope()
    val snackbarHostState = remember { SnackbarHostState() }

    var currentPath by remember(device.id) { mutableStateOf("") }
    var entries by remember(device.id) { mutableStateOf<List<RemoteStorageEntry>>(emptyList()) }
    var showHidden by remember(device.id) { mutableStateOf(false) }
    var loading by remember(device.id) { mutableStateOf(false) }
    var downloadingPath by remember(device.id) { mutableStateOf<String?>(null) }
    var refreshTick by remember(device.id) { mutableStateOf(0) }
    var listError by remember(device.id, currentPath) { mutableStateOf<String?>(null) }
    // Files picked in this folder (path → name), to download together.
    var picked by remember(device.id, currentPath) { mutableStateOf(mapOf<String, String>()) }

    // One after another: each download reports back before the next starts.
    fun downloadMany(queue: List<Pair<String, String>>, saved: Int, total: Int) {
        val next = queue.firstOrNull()
        if (next == null) {
            downloadingPath = null
            scope.launch {
                snackbarHostState.showSnackbar(
                    message = resources.getString(R.string.pc_files_saved_many, saved, total),
                    duration = SnackbarDuration.Short,
                )
            }
            return
        }
        downloadingPath = next.first
        onDownloadFile(next.first, next.second) { uri ->
            downloadMany(queue.drop(1), saved + if (uri != null) 1 else 0, total)
        }
    }

    LaunchedEffect(device.id, currentPath, refreshTick, showHidden) {
        loading = true
        listError = null
        onListFiles(currentPath) { fetched, error ->
            entries = fetched.filter { showHidden || !it.name.startsWith(".") }.sortedWith(
                compareBy<RemoteStorageEntry> { !it.isDirectoryEntry() }
                    .thenBy(String.CASE_INSENSITIVE_ORDER) { it.name },
            )
            listError = error
            loading = false
            if (error != null) {
                scope.launch {
                    snackbarHostState.showSnackbar(
                        message = error,
                        duration = SnackbarDuration.Short,
                    )
                }
            }
        }
    }

    // Names come from the PC (Descargas, Documents, etc.); do not assume English paths.
    val rootLabel = stringResource(R.string.pc_files_root)
    val breadcrumb = if (currentPath.isEmpty()) rootLabel else currentPath.replace('\\', '/')

    Box(modifier = modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 20.dp)
                .padding(top = 8.dp, bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            ScreenHeader(
                title = stringResource(R.string.pc_files_title, device.name),
                subtitle = breadcrumb,
                online = device.online,
                onBack = {
                    if (currentPath.isNotEmpty()) {
                        currentPath = parentRemotePath(currentPath)
                    } else {
                        onBack()
                    }
                },
                actions = {
                    if (currentPath.isNotEmpty()) {
                        OutlinedButton(onClick = { currentPath = parentRemotePath(currentPath) }) {
                            Text(stringResource(R.string.pc_files_up))
                        }
                        Spacer(Modifier.width(4.dp))
                    }
                    IconButton(onClick = { refreshTick++ }) {
                        Icon(
                            painterResource(R.drawable.ic_refresh),
                            contentDescription = stringResource(R.string.action_refresh),
                        )
                    }
                },
            )

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                FilterChip(
                    selected = showHidden,
                    onClick = { showHidden = !showHidden },
                    label = { Text(stringResource(R.string.pc_files_show_hidden)) },
                )
                FilterChip(
                    selected = currentPath.isEmpty(),
                    onClick = { currentPath = "" },
                    label = { Text(rootLabel) },
                )
            }

            if (picked.isNotEmpty()) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        pluralStringResource(R.plurals.pc_files_selected, picked.size, picked.size),
                        style = MaterialTheme.typography.titleSmall,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = { picked = emptyMap() }) {
                        Text(stringResource(R.string.action_cancel))
                    }
                    Button(
                        enabled = downloadingPath == null,
                        onClick = {
                            val queue = picked.toList()
                            picked = emptyMap()
                            downloadMany(queue, 0, queue.size)
                        },
                    ) {
                        Text(stringResource(R.string.pc_files_download))
                    }
                }
            } else if (entries.any { !it.isDirectoryEntry() }) {
                Text(
                    stringResource(R.string.pc_files_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            if (loading || downloadingPath != null) {
                if (LocalReducedMotion.current) {
                    LinearProgressIndicator(progress = { 0.5f }, modifier = Modifier.fillMaxWidth())
                } else {
                    LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                }
            }

            when {
                loading && entries.isEmpty() -> Text(
                    stringResource(R.string.pc_files_loading),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 16.dp),
                )
                entries.isEmpty() -> Box(
                    modifier = Modifier.fillMaxWidth().weight(1f),
                    contentAlignment = Alignment.TopCenter,
                ) {
                    val message = listError ?: stringResource(R.string.pc_files_empty)
                    Text(
                        message,
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (listError != null) {
                            MaterialTheme.colorScheme.error
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                        textAlign = TextAlign.Center,
                        modifier = Modifier.padding(vertical = 16.dp, horizontal = 8.dp),
                    )
                }
                else -> LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(bottom = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(entries, key = { it.path.ifEmpty { it.name } }) { entry ->
                        val isDir = entry.isDirectoryEntry()
                        val resolvedPath = entry.path.ifEmpty {
                            if (currentPath.isEmpty()) entry.name else "$currentPath/${entry.name}"
                        }
                        val isDownloading = downloadingPath == resolvedPath
                        val savedMessage = stringResource(R.string.pc_files_saved, entry.name)
                        val openActionLabel = stringResource(R.string.timeline_action_open)
                        val failedMessage = stringResource(R.string.pc_files_failed, entry.name)
                        val isPicked = resolvedPath in picked
                        fun togglePick() {
                            picked = if (isPicked) picked - resolvedPath else picked + (resolvedPath to entry.name)
                        }
                        Surface(
                            shape = MaterialTheme.shapes.large,
                            color = if (isPicked) {
                                MaterialTheme.colorScheme.secondaryContainer
                            } else {
                                MaterialTheme.colorScheme.surfaceContainerHigh
                            },
                            modifier = Modifier
                                .fillMaxWidth()
                                .combinedClickable(
                                    enabled = downloadingPath == null,
                                    role = Role.Button,
                                    onLongClickLabel = stringResource(R.string.pc_files_select),
                                    onLongClick = { if (!isDir) togglePick() },
                                ) {
                                    if (isDir) {
                                        currentPath = resolvedPath
                                    } else if (picked.isNotEmpty()) {
                                        togglePick()
                                    } else {
                                        downloadingPath = resolvedPath
                                        onDownloadFile(resolvedPath, entry.name) { savedUri ->
                                            downloadingPath = null
                                            scope.launch {
                                                if (savedUri != null) {
                                                    val result = snackbarHostState.showSnackbar(
                                                        message = savedMessage,
                                                        actionLabel = openActionLabel,
                                                        duration = SnackbarDuration.Short,
                                                    )
                                                    if (result == SnackbarResult.ActionPerformed) {
                                                        openSavedPcFile(context, savedUri, entry.name)
                                                    }
                                                } else {
                                                    snackbarHostState.showSnackbar(
                                                        message = failedMessage,
                                                        duration = SnackbarDuration.Short,
                                                    )
                                                }
                                            }
                                        }
                                    }
                                },
                        ) {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 16.dp, vertical = 12.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                if (picked.isNotEmpty() && !isDir) {
                                    // Decorative: the row itself is what toggles, and says so.
                                    Checkbox(checked = isPicked, onCheckedChange = null)
                                    Spacer(Modifier.width(12.dp))
                                }
                                Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
                                    Text(
                                        entry.name,
                                        style = MaterialTheme.typography.titleSmall,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                    val meta = if (isDownloading) {
                                        stringResource(R.string.pc_files_downloading, entry.name)
                                    } else if (!isDir) {
                                        Formatter.formatShortFileSize(context, entry.size.toLong())
                                    } else {
                                        null
                                    }
                                    if (meta != null) {
                                        Spacer(Modifier.height(2.dp))
                                        Text(
                                            meta,
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                    }
                                }
                                if (isDir) {
                                    Text(
                                        "›",
                                        style = MaterialTheme.typography.titleMedium,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }

        SnackbarHost(hostState = snackbarHostState, modifier = Modifier.align(Alignment.BottomCenter))
    }
}

private fun RemoteStorageEntry.isDirectoryEntry(): Boolean =
    kind.equals("dir", ignoreCase = true) ||
        kind.equals("directory", ignoreCase = true) ||
        kind.equals("folder", ignoreCase = true)

private fun parentRemotePath(path: String): String {
    val normalized = path.trim('/').replace('\\', '/')
    val slash = normalized.lastIndexOf('/')
    return if (slash <= 0) "" else normalized.substring(0, slash)
}

private fun openSavedPcFile(context: android.content.Context, uri: Uri, fileName: String) {
    runCatching {
        val mime = ReceivedFiles.mimeOf(fileName)
        val view = Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, mime)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(Intent.createChooser(view, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}
