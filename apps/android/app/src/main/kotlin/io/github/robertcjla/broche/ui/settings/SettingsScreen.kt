// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.ui.settings

import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.RadioButton
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.selectable
import io.github.robertcjla.broche.core.openDeveloperOptions

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.unit.dp
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import android.app.Activity
import io.github.robertcjla.broche.BuildConfig
import io.github.robertcjla.broche.R
import io.github.robertcjla.broche.calls.PhoneCalls
import io.github.robertcjla.broche.contacts.PhoneContacts
import io.github.robertcjla.broche.elevated.Elevated
import io.github.robertcjla.broche.elevated.PairingNotification
import io.github.robertcjla.broche.mirror.InputService
import io.github.robertcjla.broche.photos.RecentPhotos
import io.github.robertcjla.broche.sms.PhoneSms
import io.github.robertcjla.broche.storage.PhoneStorage
import io.github.robertcjla.broche.toggles.PhoneToggles
import io.github.robertcjla.broche.core.BackgroundAccess
import io.github.robertcjla.broche.core.CoreState
import io.github.robertcjla.broche.core.Device
import androidx.compose.ui.draw.drawBehind
import io.github.robertcjla.broche.notifications.NotificationListener
import io.github.robertcjla.broche.ui.guide.GuideHighlightTarget
import io.github.robertcjla.broche.ui.theme.Appearance
import io.github.robertcjla.broche.ui.theme.LocalGlassTheme
import io.github.robertcjla.broche.ui.theme.Tokens
import io.github.robertcjla.broche.ui.theme.glassModalBlur
import io.github.robertcjla.broche.ui.theme.glassModalContainerColor
import io.github.robertcjla.broche.ui.theme.liquidGlassCard
import kotlinx.coroutines.delay

@Composable
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
fun SettingsScreen(
    state: CoreState,
    appearance: Appearance,
    onAppearance: ((Appearance) -> Appearance) -> Unit,
    touchpadSensitivity: Float,
    onTouchpadSensitivity: (Float) -> Unit,
    onUnpair: (String) -> Unit,
    onPairNew: () -> Unit,
    onAccessChanged: () -> Unit,
    onSetStorageEnabled: (pcId: String, enabled: Boolean) -> Unit = { _, _ -> },
    onSetVideoQuality: (high: Boolean) -> Unit = {},
    onReadProfile: (pcId: String) -> io.github.robertcjla.broche.ffi.PermissionProfile? = { null },
    onReadToggles: (pcId: String) -> List<io.github.robertcjla.broche.ffi.DeviceToggle> = { emptyList() },
    onSetToggle: (pcId: String, name: String, enabled: Boolean) -> Unit = { _, _, _ -> },
    onApplyProfile: (pcId: String, profile: io.github.robertcjla.broche.ffi.PermissionProfile) -> Unit = { _, _ -> },
    onAddSafFolder: (Uri) -> Unit = {},
    onRemoveSafFolder: (String) -> Unit = {},
    onSetClipboardHistoryEnabled: (Boolean) -> Unit = {},
    onClearClipboardHistory: () -> Unit = {},
    suggestClipboardActions: Boolean = true,
    autoClipboard: Boolean = true,
    onSetAutoClipboard: (Boolean) -> Unit = {},
    onSetSuggestClipboardActions: (Boolean) -> Unit = {},
    onSetTimelineRetentionDays: (UInt) -> Unit = {},
    onClearTimeline: () -> Unit = {},
    onClearMessageCache: () -> Unit = {},
    onClearPhotoCache: () -> Unit = {},
    onClearReceivedFileHistory: () -> Unit = {},
    onClearEverything: () -> Unit = {},
    onToggleLocalSend: (Boolean) -> Unit = {},
    onToggleAwayMode: (Boolean) -> Unit = {},
    onRenamePhone: (String) -> Unit = {},
    onOpenWalkthrough: () -> Unit = {},
    onOpenFeaturesGuide: () -> Unit = {},
    highlightTarget: GuideHighlightTarget? = null,
    onHighlightConsumed: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    var confirmUnpair by remember { mutableStateOf<Device?>(null) }
    var renamingPhone by remember { mutableStateOf<String?>(null) }
    var confirmClearEverything by remember { mutableStateOf(false) }
    var activeCategory by rememberSaveable { mutableStateOf("") }
    var query by rememberSaveable { mutableStateOf("") }
    var pulsingSectionKey by remember { mutableStateOf<String?>(null) }
    val isGlass = LocalGlassTheme.current
    val anyDialogOpen = confirmUnpair != null || confirmClearEverything
    androidx.activity.compose.BackHandler(enabled = activeCategory.isNotEmpty()) { activeCategory = "" }

    val showAppearance = activeCategory == "appearance"
    val showDevices = activeCategory == "devices"
    val showPermissions = activeCategory == "permissions"
    val showFeatures = activeCategory == "features"

    // Keep a screen of cards composed on each side, so scrolling back does not rebuild them.
    @OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
    val listState = androidx.compose.foundation.lazy.rememberLazyListState(
        cacheWindow = androidx.compose.foundation.lazy.layout.LazyLayoutCacheWindow(aheadFraction = 1f, behindFraction = 1f),
    )
    val scope = rememberCoroutineScope()

    /** Opens a category and points at one of its sections. */
    fun openSection(category: String, sectionKey: String?) {
        activeCategory = category
        query = ""
        pulsingSectionKey = sectionKey
        scope.launch {
            delay(60)
            val index = sectionKey?.let { CATEGORY_SECTIONS[category]?.indexOf(it) } ?: -1
            runCatching { listState.scrollToItem(if (index >= 0) index + 1 else 0) }
            if (sectionKey != null) {
                delay(2200)
                if (pulsingSectionKey == sectionKey) pulsingSectionKey = null
            }
        }
    }

    androidx.compose.runtime.LaunchedEffect(highlightTarget) {
        val target = highlightTarget ?: return@LaunchedEffect
        val mapping = when (target) {
            GuideHighlightTarget.SETTINGS_DEVICES -> "devices" to "devices"
            GuideHighlightTarget.SETTINGS_NOTIFICATIONS -> "permissions" to "notifications"
            GuideHighlightTarget.SETTINGS_CONTROL -> "permissions" to "control"
            GuideHighlightTarget.SETTINGS_BACKGROUND -> "permissions" to "background"
            GuideHighlightTarget.SETTINGS_CALLS_CONTACTS -> "permissions" to "calls_contacts"
            GuideHighlightTarget.SETTINGS_PHOTOS_STORAGE -> "permissions" to "photos"
            GuideHighlightTarget.SETTINGS_CLIPBOARD -> "features" to "clipboard"
            GuideHighlightTarget.SETTINGS_LOCALSEND -> "features" to "localsend"
            GuideHighlightTarget.SETTINGS_DATA_STORAGE -> "data" to "data_storage"
            else -> null
        }
        onHighlightConsumed()
        if (mapping != null) openSection(mapping.first, mapping.second)
    }

    // What each permission group has, for the summaries and the status chips.
    val permissionStatus = linkedMapOf(
        "notifications" to state.notificationAccess,
        "control" to (state.inputAccess || state.elevated),
        "system" to state.dndAccess,
        "background" to state.backgroundUnrestricted,
        "calls_contacts" to (state.callAccess && state.contactsAccess && state.smsAccess),
        "photos" to (state.photoAccess && (state.storageAllFilesAccess || state.storageSafFolders.isNotEmpty())),
    )
    val categories = listOf(
        SettingsCategory("devices", R.drawable.ic_devices, stringResource(R.string.settings_category_devices),
            stringResource(R.string.settings_cat_devices_summary, state.devices.size)),
        SettingsCategory("permissions", R.drawable.ic_shield, stringResource(R.string.settings_cat_android_permissions),
            stringResource(R.string.settings_cat_permissions_summary, permissionStatus.values.count { it }, permissionStatus.size)),
        SettingsCategory("features", R.drawable.ic_tune, stringResource(R.string.settings_category_features),
            stringResource(R.string.settings_cat_features_summary)),
        SettingsCategory("appearance", R.drawable.ic_palette, stringResource(R.string.settings_appearance),
            stringResource(R.string.settings_cat_appearance_summary)),
        SettingsCategory("video", R.drawable.ic_videocam, stringResource(R.string.settings_video),
            stringResource(R.string.settings_cat_video_summary)),
        SettingsCategory("data", R.drawable.ic_storage, stringResource(R.string.settings_data_storage_title),
            stringResource(R.string.settings_cat_data_summary)),
        SettingsCategory("about", R.drawable.ic_info, stringResource(R.string.settings_about),
            stringResource(R.string.settings_cat_about_summary)),
    )
    val searchIndex = settingsSearchIndex()

    LazyColumn(
        state = listState,
        modifier = modifier.glassModalBlur(isGlass && anyDialogOpen),
        contentPadding = PaddingValues(start = 16.dp, top = 16.dp, end = 16.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        if (activeCategory.isEmpty()) {
            item(key = "header", contentType = "header") {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Text(
                            stringResource(R.string.settings_title),
                            style = MaterialTheme.typography.headlineMedium,
                            modifier = Modifier.semantics { heading() },
                        )
                        TextButton(onClick = onOpenFeaturesGuide) {
                            Text(stringResource(R.string.guide_title))
                        }
                    }
                    SettingsSearchField(query = query, onQuery = { query = it })
                }
            }
            if (query.isBlank()) {
                item(key = "categories", contentType = "categories") {
                    androidx.compose.material3.OutlinedCard(Modifier.fillMaxWidth()) {
                        categories.forEachIndexed { i, cat ->
                            if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                            val warn = cat.id == "permissions" && permissionStatus.values.any { !it }
                            CategoryRow(cat, warn) { openSection(cat.id, null) }
                        }
                    }
                }
            } else {
                val q = normalize(query)
                val results = searchIndex.filter { e -> q.split(' ').filter { it.isNotBlank() }.all { normalize(e.text).contains(it) } }
                item(key = "results", contentType = "results") {
                    if (results.isEmpty()) {
                        Text(
                            stringResource(R.string.settings_search_empty, query.trim()),
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(8.dp),
                        )
                    } else {
                        androidx.compose.material3.OutlinedCard(Modifier.fillMaxWidth()) {
                            results.forEachIndexed { i, r ->
                                if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                                val cat = categories.first { it.id == r.category }
                                androidx.compose.material3.ListItem(
                                    headlineContent = { Text(r.title) },
                                    supportingContent = { Text(cat.title) },
                                    leadingContent = { CategoryIcon(cat.icon) },
                                    colors = androidx.compose.material3.ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                                    modifier = Modifier.clickable(onClickLabel = cat.title) { openSection(r.category, r.key) },
                                )
                            }
                        }
                    }
                }
            }
        } else {
            val cat = categories.firstOrNull { it.id == activeCategory }
            item(key = "category_header", contentType = "header") {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 4.dp)) {
                    androidx.compose.material3.IconButton(onClick = { activeCategory = "" }) {
                        androidx.compose.material3.Icon(
                            androidx.compose.ui.res.painterResource(R.drawable.ic_back),
                            contentDescription = stringResource(R.string.settings_back),
                        )
                    }
                    Column(Modifier.weight(1f)) {
                        Text(
                            cat?.title.orEmpty(),
                            style = MaterialTheme.typography.headlineSmall,
                            modifier = Modifier.semantics { heading() },
                        )
                        if (cat != null) {
                            Text(cat.summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }

        if (showAppearance) {
            item(key = "appearance", contentType = "section") {
                Section(stringResource(R.string.settings_appearance), highlighted = pulsingSectionKey == "appearance") {
                    Label(stringResource(R.string.settings_theme))
                    val themeChoices = io.github.robertcjla.broche.ui.theme.SCHEME_CHOICES +
                        ("system" to stringResource(R.string.theme_system_colors))
                    var showThemeMenu by remember { mutableStateOf(false) }
                    androidx.compose.foundation.layout.Box {
                        OutlinedButton(onClick = { showThemeMenu = true }, modifier = Modifier.fillMaxWidth()) {
                            Text(themeChoices.firstOrNull { it.first == appearance.theme }?.second ?: themeChoices[0].second)
                        }
                        androidx.compose.material3.DropdownMenu(expanded = showThemeMenu, onDismissRequest = { showThemeMenu = false }) {
                            themeChoices.forEach { (id, label) ->
                                androidx.compose.material3.DropdownMenuItem(text = { Text(label) }, onClick = {
                                    onAppearance { it.copy(theme = id) }
                                    showThemeMenu = false
                                })
                            }
                        }
                    }
                    if (Build.VERSION.SDK_INT >= 33) {
                        val context = LocalContext.current
                        val localeManager = context.getSystemService(android.app.LocaleManager::class.java)
                        val chosen = localeManager.applicationLocales.let { if (it.isEmpty) "" else it[0].language }
                        Label(stringResource(R.string.settings_language))
                        // Each language is named in itself, so it can be found in any language.
                        Choice(
                            listOf("" to stringResource(R.string.language_system), "en" to "English", "es" to "Español"),
                            chosen,
                        ) { tag ->
                            // The system restarts the screen in the new language and keeps the choice.
                            localeManager.applicationLocales =
                                if (tag.isEmpty()) android.os.LocaleList.getEmptyLocaleList() else android.os.LocaleList.forLanguageTags(tag)
                        }
                    }
                    Label(stringResource(R.string.settings_mode))
                    Choice(
                        listOf(
                            "system" to stringResource(R.string.mode_system),
                            "light" to stringResource(R.string.mode_light),
                            "dark" to stringResource(R.string.mode_dark),
                        ),
                        appearance.mode,
                    ) { value -> onAppearance { it.copy(mode = value) } }
                }
            }
        }

        if (showDevices) {
            item(key = "devices", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_devices),
                    highlighted = pulsingSectionKey == "devices",
                ) {
                    val context = LocalContext.current
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(stringResource(R.string.phone_name_title), style = MaterialTheme.typography.titleSmall)
                            Text(
                                state.phoneName,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        OutlinedButton(onClick = { renamingPhone = state.phoneName }) {
                            Text(stringResource(R.string.action_change))
                        }
                    }
                    SettingRowDivider()
                    state.devices.forEachIndexed { index, device ->
                        if (index > 0) {
                            SettingRowDivider()
                        }
                        Column(verticalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.fillMaxWidth()) {
                            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                                Column(Modifier.weight(1f).padding(end = 12.dp)) {
                                    Text(device.name, style = MaterialTheme.typography.titleSmall)
                                    val dotColor = if (device.online) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                                    ) {
                                        Spacer(
                                            modifier = Modifier
                                                .size(8.dp)
                                                .drawBehind { drawCircle(dotColor) },
                                        )
                                        Text(
                                            stringResource(if (device.online) R.string.connected else R.string.not_connected),
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        )
                                    }
                                }
                            }
                            // A permission change can turn phone storage on: ask for file access then,
                            // as the storage switch did before it moved into the profile.
                            var storageBefore by remember(device.id) { mutableStateOf(device.storageEnabled) }
                            LaunchedEffect(device.storageEnabled) {
                                if (device.storageEnabled && !storageBefore &&
                                    !state.storageAllFilesAccess && state.storageSafFolders.isEmpty()
                                ) {
                                    runCatching { context.startActivity(PhoneStorage.allFilesAccessIntent(context)) }
                                }
                                storageBefore = device.storageEnabled
                            }
                            var revision by remember(device.id) { mutableIntStateOf(0) }
                            val profile = remember(device.id, device.storageEnabled, revision) { onReadProfile(device.id) }
                            val toggles = remember(device.id, device.storageEnabled, revision) { onReadToggles(device.id) }
                            Text(stringResource(R.string.profile_title), style = MaterialTheme.typography.bodyMedium)
                            PermissionProfilePicker(profile, onSelect = {
                                onApplyProfile(device.id, it)
                                revision++
                            })
                            DetailedPermissions(toggles, custom = profile == null, onToggle = { name, enabled ->
                                onSetToggle(device.id, name, enabled)
                                revision++
                            })
                            OutlinedButton(
                                onClick = { confirmUnpair = device },
                                colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
                            ) { Text(stringResource(R.string.action_unpair)) }
                        }
                    }
                    FilledTonalButton(onClick = onPairNew) { Text(stringResource(R.string.action_pair_new)) }
                    SettingRowDivider()
                    val mobileDataTitle = stringResource(R.string.guide_away_title)
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(mobileDataTitle, style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.settings_mobile_data_body),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Switch(
                            checked = state.awayMode,
                            onCheckedChange = onToggleAwayMode,
                            colors = io.github.robertcjla.broche.ui.theme.brocheSwitchColors(),
                            modifier = Modifier.semantics { contentDescription = mobileDataTitle },
                        )
                    }
                }
            }
        }

        if (showPermissions) {
            item(key = "perm_intro", contentType = "intro") {
                Surface(
                    shape = MaterialTheme.shapes.large,
                    color = MaterialTheme.colorScheme.secondaryContainer,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        stringResource(R.string.settings_permissions_intro),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }
            item(key = "notifications", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_notifications),
                    highlighted = pulsingSectionKey == "notifications",
                    description = stringResource(R.string.perm_desc_notifications),
                    granted = permissionStatus["notifications"],
                ) {
                    NotificationAccess(state.notificationAccess)
                }
            }

            item(key = "control", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_control),
                    highlighted = pulsingSectionKey == "control",
                    description = stringResource(R.string.perm_desc_control),
                    granted = permissionStatus["control"],
                ) {
                    ControlAccess(state.inputAccess)
                    SettingRowDivider()
                    var advanced by rememberSaveable { mutableStateOf(false) }
                    androidx.compose.material3.TextButton(onClick = { advanced = !advanced }) {
                        Text(stringResource(if (advanced) R.string.advanced_hide else R.string.advanced_show))
                    }
                    if (advanced) {
                        ElevatedAccess()
                    }
                }
            }

            item(key = "system", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_system_title),
                    highlighted = pulsingSectionKey == "system",
                    description = stringResource(R.string.perm_desc_system),
                    granted = permissionStatus["system"],
                ) {
                    DndAccess(state.dndAccess)
                }
            }

            item(key = "background", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_background_title),
                    highlighted = pulsingSectionKey == "background",
                    description = stringResource(R.string.perm_desc_background),
                    granted = permissionStatus["background"],
                ) {
                    BackgroundBatteryAccess(state.backgroundUnrestricted)
                }
            }

            item(key = "calls_contacts", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_calls_and_contacts),
                    highlighted = pulsingSectionKey == "calls_contacts",
                    description = stringResource(R.string.perm_desc_calls),
                    granted = permissionStatus["calls_contacts"],
                ) {
                    RuntimePermissionAccess(
                        title = stringResource(R.string.calls_title),
                        textOn = stringResource(R.string.calls_on),
                        textOff = stringResource(R.string.calls_off),
                        granted = state.callAccess,
                        permissions = PhoneCalls.permissions,
                        onAccessChanged = onAccessChanged,
                    )
                    SettingRowDivider()
                    // Long steps: full width, with the button under the text (a side button left a narrow column).
                    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(stringResource(R.string.calls_bluetooth_title), style = MaterialTheme.typography.titleSmall)
                        Text(
                            stringResource(R.string.calls_bluetooth_help),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        val context = LocalContext.current
                        OutlinedButton(onClick = {
                            runCatching { context.startActivity(Intent(Settings.ACTION_BLUETOOTH_SETTINGS)) }
                        }) { Text(stringResource(R.string.calls_bluetooth_open)) }
                    }
                    SettingRowDivider()
                    RuntimePermissionAccess(
                        title = stringResource(R.string.contacts_title),
                        textOn = stringResource(R.string.contacts_on),
                        textOff = stringResource(R.string.contacts_off),
                        granted = state.contactsAccess,
                        permissions = PhoneContacts.permissions,
                        onAccessChanged = onAccessChanged,
                    )
                    SettingRowDivider()
                    RuntimePermissionAccess(
                        title = stringResource(R.string.sms_title),
                        textOn = stringResource(R.string.sms_on),
                        textOff = stringResource(R.string.sms_off),
                        granted = state.smsAccess,
                        permissions = PhoneSms.permissions,
                        onAccessChanged = onAccessChanged,
                    )
                }
            }

            item(key = "photos", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_photos),
                    highlighted = pulsingSectionKey == "photos",
                    description = stringResource(R.string.perm_desc_photos),
                    granted = permissionStatus["photos"],
                ) {
                    PhotosAccess(
                        granted = state.photoAccess,
                        partial = state.photoPartialAccess,
                        onAccessChanged = onAccessChanged,
                    )
                    SettingRowDivider()
                    StorageAccess(
                        allFiles = state.storageAllFilesAccess,
                        safFolders = state.storageSafFolders,
                        onAddSafFolder = onAddSafFolder,
                        onRemoveSafFolder = onRemoveSafFolder,
                    )
                }
            }
        }

        if (showFeatures) {
            item(key = "clipboard", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_clipboard_title),
                    highlighted = pulsingSectionKey == "clipboard",
                ) {
                    val context = LocalContext.current
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(stringResource(R.string.settings_background_clipboard_title), style = MaterialTheme.typography.titleSmall)
                            Text(
                                if (state.elevated) {
                                    stringResource(R.string.settings_wireless_debugging_active)
                                } else {
                                    stringResource(R.string.settings_wireless_debugging_hint)
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        if (state.elevated) {
                            OutlinedButton(onClick = { openDeveloperOptions(context) }) {
                                Text(stringResource(R.string.action_manage))
                            }
                        } else {
                            Button(onClick = {
                                PairingNotification.show(context)
                                openDeveloperOptions(context)
                            }) {
                                Text(stringResource(R.string.action_allow))
                            }
                        }
                    }
                    SettingRowDivider()
                    val autoTitle = stringResource(R.string.settings_auto_clipboard)
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(autoTitle, style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.settings_auto_clipboard_hint),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Switch(
                            checked = autoClipboard,
                            onCheckedChange = onSetAutoClipboard,
                            colors = io.github.robertcjla.broche.ui.theme.brocheSwitchColors(),
                            modifier = Modifier.semantics { contentDescription = autoTitle },
                        )
                    }
                    SettingRowDivider()
                    val suggestionsTitle = stringResource(R.string.settings_clipboard_suggestions)
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(suggestionsTitle, style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.settings_clipboard_suggestions_hint),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Switch(
                            checked = suggestClipboardActions,
                            onCheckedChange = onSetSuggestClipboardActions,
                            colors = io.github.robertcjla.broche.ui.theme.brocheSwitchColors(),
                            modifier = Modifier.semantics { contentDescription = suggestionsTitle },
                        )
                    }
                }
            }

            item(key = "remote", contentType = "section") {
                Section(stringResource(R.string.settings_remote), highlighted = pulsingSectionKey == "remote") {
                    val sensitivityTitle = stringResource(R.string.settings_touchpad_sensitivity)
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Text(
                            sensitivityTitle,
                            style = MaterialTheme.typography.titleSmall,
                            modifier = Modifier.weight(1f),
                        )
                        Text(
                            stringResource(R.string.settings_touchpad_sensitivity_value, touchpadSensitivity),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Slider(
                        value = touchpadSensitivity,
                        onValueChange = onTouchpadSensitivity,
                        valueRange = 0.5f..2.5f,
                        modifier = Modifier.semantics { contentDescription = sensitivityTitle },
                    )
                }
            }

            item(key = "localsend", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_localsend_title),
                    highlighted = pulsingSectionKey == "localsend",
                ) {
                    val localsendTitle = stringResource(R.string.settings_localsend_title)
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(localsendTitle, style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.settings_localsend_subtitle),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Switch(
                            checked = state.localsendEnabled,
                            onCheckedChange = onToggleLocalSend,
                            colors = io.github.robertcjla.broche.ui.theme.brocheSwitchColors(),
                            modifier = Modifier.semantics { contentDescription = localsendTitle },
                        )
                    }
                }
            }

        }
        if (activeCategory == "data") {
            item(key = "data_storage", contentType = "section") {
                Section(
                    title = stringResource(R.string.settings_data_storage_title),
                    highlighted = pulsingSectionKey == "data_storage",
                ) {
                    // 1. Notification history
                    Column(Modifier.fillMaxWidth()) {
                        Text(stringResource(R.string.data_notifications_title), style = MaterialTheme.typography.titleSmall)
                        Text(
                            stringResource(R.string.data_notifications_subtitle),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    SettingRowDivider()

                    // 2. Clipboard history
                    val clipboardHistoryTitle = stringResource(R.string.data_clipboard_title)
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(clipboardHistoryTitle, style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.data_clipboard_subtitle),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            if (state.clipboardHistory.isNotEmpty()) {
                                Text(
                                    pluralStringResource(
                                        R.plurals.data_clipboard_count,
                                        state.clipboardHistory.size,
                                        state.clipboardHistory.size,
                                    ),
                                    style = MaterialTheme.typography.labelMedium,
                                    color = MaterialTheme.colorScheme.primary,
                                    modifier = Modifier.padding(top = 2.dp),
                                )
                            }
                        }
                        Switch(
                            checked = state.clipboardHistoryEnabled,
                            onCheckedChange = onSetClipboardHistoryEnabled,
                            colors = io.github.robertcjla.broche.ui.theme.brocheSwitchColors(),
                            modifier = Modifier.semantics { contentDescription = clipboardHistoryTitle },
                        )
                    }
                    if (state.clipboardHistory.isNotEmpty()) {
                        OutlinedButton(onClick = onClearClipboardHistory) {
                            Text(stringResource(R.string.data_action_clear))
                        }
                    }
                    SettingRowDivider()

                    // 3. Timeline
                    Text(stringResource(R.string.data_timeline_title), style = MaterialTheme.typography.titleSmall)
                    Text(
                        stringResource(R.string.data_timeline_subtitle),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Choice(
                        listOf(
                            "30" to stringResource(R.string.timeline_retention_30d),
                            "90" to stringResource(R.string.timeline_retention_90d),
                            "365" to stringResource(R.string.timeline_retention_1y),
                            "0" to stringResource(R.string.timeline_retention_all),
                        ),
                        state.timelineRetentionDays.toString(),
                    ) { value -> value.toUIntOrNull()?.let(onSetTimelineRetentionDays) }
                    if (state.timeline.isNotEmpty() || state.timelineTotal > 0u) {
                        OutlinedButton(onClick = onClearTimeline) {
                            Text(stringResource(R.string.data_action_clear))
                        }
                    }
                    SettingRowDivider()

                    // 4. Message cache
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(stringResource(R.string.data_messages_title), style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.data_messages_subtitle),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        if (state.cachedMmsFiles > 0) {
                            OutlinedButton(onClick = onClearMessageCache) {
                                Text(stringResource(R.string.data_action_clear))
                            }
                        }
                    }
                    SettingRowDivider()

                    // 5. Photo thumbnails & camera cache
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(stringResource(R.string.data_photos_title), style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.data_photos_subtitle),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        if (state.cachedPhotoFiles > 0) {
                            OutlinedButton(onClick = onClearPhotoCache) {
                                Text(stringResource(R.string.data_action_clear))
                            }
                        }
                    }
                    SettingRowDivider()

                    // 6. Received-file history
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(stringResource(R.string.data_received_files_title), style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.data_received_files_subtitle),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        if (state.receivedFileRecords > 0 || state.transfers.isNotEmpty()) {
                            OutlinedButton(onClick = onClearReceivedFileHistory) {
                                Text(stringResource(R.string.data_action_clear))
                            }
                        }
                    }
                    SettingRowDivider()

                    // 7. Clear everything
                    Text(
                        stringResource(R.string.data_clear_everything_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    OutlinedButton(onClick = { confirmClearEverything = true }) {
                        Text(stringResource(R.string.data_clear_everything_button))
                    }
                }
            }
        }

        if (activeCategory == "video") {
            item(key = "video", contentType = "section") {
                Section(stringResource(R.string.settings_video), highlighted = pulsingSectionKey == "video") {
                    val context = LocalContext.current
                    var high by remember { mutableStateOf(io.github.robertcjla.broche.video.VideoPreferences.high(context)) }
                    Text(stringResource(R.string.video_quality_hint), style = MaterialTheme.typography.bodyMedium)
                    SettingRowDivider()
                    Column(Modifier.selectableGroup()) {
                        listOf(false to R.string.video_quality_normal, true to R.string.video_quality_high).forEach { (value, label) ->
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .selectable(selected = high == value, role = Role.RadioButton) {
                                        high = value
                                        onSetVideoQuality(value)
                                    }
                                    .padding(vertical = 8.dp),
                            ) {
                                RadioButton(selected = high == value, onClick = null)
                                Text(stringResource(label), modifier = Modifier.padding(start = 12.dp))
                            }
                        }
                    }
                    Text(
                        stringResource(R.string.video_quality_high_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        if (activeCategory == "about") {
            item(key = "about", contentType = "section") {
                Section(stringResource(R.string.settings_about), highlighted = pulsingSectionKey == "about") {
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(stringResource(R.string.guide_title), style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.settings_guide_card_body),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        FilledTonalButton(onClick = onOpenFeaturesGuide) {
                            Text(stringResource(R.string.clip_action_open))
                        }
                    }
                    SettingRowDivider()
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(stringResource(R.string.guide_open_walkthrough), style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.settings_walkthrough_body),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        OutlinedButton(onClick = onOpenWalkthrough) {
                            Text(stringResource(R.string.clip_action_open))
                        }
                    }
                    SettingRowDivider()
                    val uriHandler = LocalUriHandler.current
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(stringResource(R.string.about_source_title), style = MaterialTheme.typography.titleSmall)
                            Text(
                                stringResource(R.string.about_source_body),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        // No browser installed would throw: nothing to open it with, so nothing happens.
                        OutlinedButton(onClick = { runCatching { uriHandler.openUri(SOURCE_URL) } }) {
                            Text(stringResource(R.string.clip_action_open))
                        }
                    }
                    SettingRowDivider()
                    Text(stringResource(R.string.about_version, BuildConfig.VERSION_NAME), style = MaterialTheme.typography.bodyMedium)
                    Text(stringResource(R.string.about_fork), style = MaterialTheme.typography.bodyMedium)
                    Text("GPL-3.0-or-later · MPL-2.0", style = MaterialTheme.typography.labelLarge)
                    Text(stringResource(R.string.about_brand_plan), style = MaterialTheme.typography.bodySmall)
                }
            }
        }
    }

    renamingPhone?.let { draft ->
        AlertDialog(
            onDismissRequest = { renamingPhone = null },
            containerColor = glassModalContainerColor(),
            title = { Text(stringResource(R.string.phone_name_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.phone_name_body))
                    OutlinedTextField(
                        value = draft,
                        onValueChange = { renamingPhone = it.take(40) },
                        singleLine = true,
                        label = { Text(stringResource(R.string.phone_name_title)) },
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = { onRenamePhone(draft); renamingPhone = null }) {
                    Text(stringResource(R.string.action_save))
                }
            },
            dismissButton = {
                TextButton(onClick = { renamingPhone = null }) { Text(stringResource(R.string.action_cancel)) }
            },
        )
    }

    confirmUnpair?.let { device ->
        AlertDialog(
            onDismissRequest = { confirmUnpair = null },
            containerColor = glassModalContainerColor(),
            title = { Text(stringResource(R.string.unpair_title, device.name)) },
            text = { Text(stringResource(R.string.unpair_text)) },
            confirmButton = {
                TextButton(onClick = { onUnpair(device.id); confirmUnpair = null }) {
                    Text(stringResource(R.string.action_unpair))
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmUnpair = null }) { Text(stringResource(R.string.action_cancel)) }
            },
        )
    }

    if (confirmClearEverything) {
        AlertDialog(
            onDismissRequest = { confirmClearEverything = false },
            containerColor = glassModalContainerColor(),
            title = { Text(stringResource(R.string.data_clear_everything_dialog_title)) },
            text = { Text(stringResource(R.string.data_clear_everything_dialog_body)) },
            confirmButton = {
                TextButton(onClick = {
                    onClearEverything()
                    confirmClearEverything = false
                }) {
                    Text(stringResource(R.string.data_clear_everything_confirm))
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmClearEverything = false }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }
}

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is android.content.ContextWrapper -> baseContext.findActivity()
    else -> null
}

private fun openAppInfoSettings(context: Context) {
    runCatching {
        context.startActivity(
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    }
}



/**
 * Notification access, which mirroring needs. Android shows the switch in
 * its own settings; sideloaded apps on Android 13+ must first be allowed
 * "restricted settings" there.
 */
@Composable
private fun NotificationAccess(granted: Boolean) {
    val context = LocalContext.current
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(stringResource(R.string.notifications_title), style = MaterialTheme.typography.titleSmall)
            Text(
                stringResource(if (granted) R.string.notifications_on else R.string.notifications_off),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        val open = { runCatching { context.startActivity(NotificationListener.settingsIntent(context)) }.let {} }
        if (granted) {
            OutlinedButton(onClick = open) { Text(stringResource(R.string.action_manage)) }
        } else {
            Button(onClick = open) { Text(stringResource(R.string.action_allow)) }
        }
    }
    if (!granted && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Text(
                stringResource(R.string.notifications_restricted),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = { openAppInfoSettings(context) }) { Text(stringResource(R.string.action_app_info)) }
        }
    }
}

/** Unrestricted background execution and OEM-specific battery settings. */
@Composable
private fun BackgroundBatteryAccess(unrestricted: Boolean) {
    val context = LocalContext.current
    val oemGuide = BackgroundAccess.oemGuideResId()?.let { stringResource(it) }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                Text(stringResource(R.string.background_title), style = MaterialTheme.typography.titleSmall)
                Text(
                    stringResource(if (unrestricted) R.string.background_on else R.string.background_text),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (!unrestricted) {
                Button(onClick = {
                    runCatching { context.startActivity(BackgroundAccess.requestIntent(context)) }
                }) {
                    Text(stringResource(R.string.action_allow))
                }
            } else {
                OutlinedButton(onClick = {
                    runCatching { context.startActivity(BackgroundAccess.oemSettingsIntent(context)) }
                }) {
                    Text(stringResource(R.string.battery_open_oem_settings))
                }
            }
        }
        if (oemGuide != null) {
            Text(
                oemGuide,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (!unrestricted) {
            OutlinedButton(onClick = {
                runCatching { context.startActivity(BackgroundAccess.oemSettingsIntent(context)) }
            }) {
                Text(stringResource(R.string.battery_open_oem_settings))
            }
        }
    }
}

/** Do Not Disturb and silent ringer control from a PC. */
@Composable
private fun DndAccess(granted: Boolean) {
    val context = LocalContext.current
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(stringResource(R.string.dnd_access_title), style = MaterialTheme.typography.titleSmall)
            Text(
                stringResource(if (granted) R.string.dnd_access_on else R.string.dnd_access_off),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        val open = { runCatching { context.startActivity(PhoneToggles.dndSettingsIntent()) }.let {} }
        if (granted) {
            OutlinedButton(onClick = open) { Text(stringResource(R.string.action_manage)) }
        } else {
            Button(onClick = open) { Text(stringResource(R.string.action_allow)) }
        }
    }
}

/** Control from the PC: Broche's accessibility service, turned on in Android's settings. */
@Composable
private fun ControlAccess(on: Boolean) {
    val context = LocalContext.current
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(stringResource(R.string.control_title), style = MaterialTheme.typography.titleSmall)
            Text(
                stringResource(if (on) R.string.control_on else R.string.control_off),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        val open = { runCatching { context.startActivity(InputService.settingsIntent()) }.let {} }
        if (on) {
            OutlinedButton(onClick = open) { Text(stringResource(R.string.action_manage)) }
        } else {
            Button(onClick = open) { Text(stringResource(R.string.action_allow)) }
        }
    }
    if (!on && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Text(
                stringResource(R.string.notifications_restricted),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f).padding(end = 12.dp),
            )
            OutlinedButton(onClick = { openAppInfoSettings(context) }) { Text(stringResource(R.string.action_app_info)) }
        }
    }
}

/**
 * An app-op advanced mode can grant, only when the user turns this on (off by default; Broche
 * never grants it by itself). Reads the real state back, since the helper applies it a moment later.
 */
@Composable
private fun UserAppOpSwitch(title: Int, body: Int, op: String, isAllowed: (android.content.Context) -> Boolean) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var on by remember { mutableStateOf(isAllowed(context)) }
    // The whole row is the switch, so screen readers read its title, text and state together.
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .toggleable(value = on, role = Role.Switch) { wanted ->
                if (!Elevated.setAppOp(op, wanted)) return@toggleable
                on = wanted
                scope.launch {
                    kotlinx.coroutines.delay(APP_OP_SETTLE_MS)
                    on = isAllowed(context)
                    // PCs learn what this phone can do now (notify.sensitive, call.incall).
                    (context.applicationContext as io.github.robertcjla.broche.BrocheApplication).core.refreshNotificationAccess()
                }
            },
    ) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(stringResource(title), style = MaterialTheme.typography.titleSmall)
            Text(
                stringResource(body),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Switch(checked = on, onCheckedChange = null, colors = io.github.robertcjla.broche.ui.theme.brocheSwitchColors())
    }
}

private const val APP_OP_SETTLE_MS = 800L

/** Elevated through the phone's own wireless debugging: real touch and keys. */
@Composable
private fun ElevatedAccess() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state by Elevated.state.collectAsStateWithLifecycle()
    var steps by remember { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                Text(stringResource(R.string.elevated_title), style = MaterialTheme.typography.titleSmall)
                Text(
                    stringResource(
                        when (state) {
                            Elevated.State.NotSetUp -> R.string.elevated_not_set_up
                            Elevated.State.Waiting -> R.string.elevated_waiting
                            Elevated.State.Starting -> R.string.elevated_starting
                            Elevated.State.Running ->
                                if (Elevated.rootActive) R.string.elevated_running_root else R.string.elevated_running
                            is Elevated.State.Failed -> when ((state as Elevated.State.Failed).reason) {
                                Elevated.Reason.NotPaired -> R.string.elevated_not_paired
                                Elevated.Reason.RootDenied -> R.string.elevated_root_failed
                                else -> R.string.elevated_failed
                            }
                        },
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            when (state) {
                Elevated.State.NotSetUp, is Elevated.State.Failed -> Button(onClick = { steps = true }) {
                    Text(stringResource(R.string.elevated_set_up))
                }
                Elevated.State.Waiting -> Button(onClick = {
                    scope.launch {
                        PairingNotification.show(context)
                        if (!Elevated.start()) {
                            openDeveloperOptions(context)
                        }
                    }
                }) {
                    Text(stringResource(R.string.elevated_try_again))
                }
                Elevated.State.Running -> OutlinedButton(onClick = { scope.launch { Elevated.forget() } }) {
                    Text(stringResource(R.string.elevated_turn_off))
                }
                Elevated.State.Starting -> {}
            }
        }
        if (state == Elevated.State.Waiting || state is Elevated.State.Failed) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = {
                    PairingNotification.show(context)
                    openDeveloperOptions(context)
                }) {
                    Text(stringResource(R.string.elevated_open_developer))
                }
            }
        }
        if (state == Elevated.State.Running) {
            // Android 12+ binds call companions; Android 15+ hides sensitive notifications.
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                UserAppOpSwitch(
                    R.string.elevated_calls_title,
                    R.string.elevated_calls_body,
                    io.github.robertcjla.broche.calls.CallCompanion.OP_NAME,
                    io.github.robertcjla.broche.calls.CallCompanion::allowed,
                )
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM) {
                UserAppOpSwitch(
                    R.string.elevated_sensitive_title,
                    R.string.elevated_sensitive_body,
                    NotificationListener.OP_SENSITIVE_NAME,
                    NotificationListener::canReadSensitive,
                )
            }
        }
        // Root only when the user picks it, and only on a rooted phone.
        val rootOffered = Elevated.isRootAvailable() &&
            (state == Elevated.State.NotSetUp || state is Elevated.State.Failed)
        if (rootOffered) {
            Text(
                stringResource(R.string.elevated_root_hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedButton(onClick = { scope.launch { Elevated.setUpWithRoot() } }) {
                Text(stringResource(R.string.elevated_use_root))
            }
        }
    }
    if (steps) {
        AlertDialog(
            onDismissRequest = { steps = false },
            title = { Text(stringResource(R.string.elevated_steps_title)) },
            text = { Text(stringResource(R.string.elevated_steps)) },
            confirmButton = {
                TextButton(onClick = {
                    steps = false
                    PairingNotification.show(context)
                    openDeveloperOptions(context)
                }) { Text(stringResource(R.string.elevated_open_developer)) }
            },
            dismissButton = {
                TextButton(onClick = { steps = false }) { Text(stringResource(R.string.action_cancel)) }
            },
        )
    }
}

@Composable
private fun RuntimePermissionAccess(
    title: String,
    textOn: String,
    textOff: String,
    granted: Boolean,
    permissions: Array<String>,
    onAccessChanged: () -> Unit,
) {
    val context = LocalContext.current
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { grants ->
        onAccessChanged()
        if (grants.values.none { it }) {
            openAppInfoSettings(context)
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Text(
                if (granted) textOn else textOff,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (granted) {
            OutlinedButton(onClick = { openAppInfoSettings(context) }) {
                Text(stringResource(R.string.action_manage))
            }
        } else {
            Button(onClick = { ask.launch(permissions) }) { Text(stringResource(R.string.action_allow)) }
        }
    }
}

@Composable
private fun PhotosAccess(
    granted: Boolean,
    partial: Boolean,
    onAccessChanged: () -> Unit,
) {
    val context = LocalContext.current
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { grants ->
        onAccessChanged()
        if (grants.values.none { it }) {
            openAppInfoSettings(context)
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(stringResource(R.string.photos_title), style = MaterialTheme.typography.titleSmall)
            Text(
                stringResource(
                    when {
                        partial -> R.string.photos_partial
                        granted -> R.string.photos_on
                        else -> R.string.photos_off
                    },
                ),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        when {
            partial -> OutlinedButton(onClick = { ask.launch(RecentPhotos.permissions) }) {
                Text(stringResource(R.string.action_select_photos))
            }
            granted -> OutlinedButton(onClick = { openAppInfoSettings(context) }) {
                Text(stringResource(R.string.action_manage))
            }
            else -> Button(onClick = { ask.launch(RecentPhotos.permissions) }) {
                Text(stringResource(R.string.action_allow))
            }
        }
    }
}

@Composable
private fun StorageAccess(
    allFiles: Boolean,
    safFolders: List<String>,
    onAddSafFolder: (Uri) -> Unit,
    onRemoveSafFolder: (String) -> Unit,
) {
    val context = LocalContext.current
    val pickFolder = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        if (uri != null) onAddSafFolder(uri)
    }
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(stringResource(R.string.storage_all_files_title), style = MaterialTheme.typography.titleSmall)
            Text(
                stringResource(if (allFiles) R.string.storage_all_files_on else R.string.storage_all_files_off),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        val open = { runCatching { context.startActivity(PhoneStorage.allFilesAccessIntent(context)) }.let {} }
        if (allFiles) {
            OutlinedButton(onClick = open) { Text(stringResource(R.string.action_manage)) }
        } else {
            Button(onClick = open) { Text(stringResource(R.string.action_allow)) }
        }
    }
    if (!allFiles) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                Text(stringResource(R.string.storage_saf_title), style = MaterialTheme.typography.titleSmall)
                Text(
                    stringResource(R.string.storage_saf_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            OutlinedButton(onClick = { pickFolder.launch(null) }) {
                Text(stringResource(R.string.storage_add_folder))
            }
        }
        safFolders.forEach { name ->
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                Text(name, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                TextButton(onClick = { onRemoveSafFolder(name) }) {
                    Text(stringResource(R.string.storage_remove_folder))
                }
            }
        }
    }
}

@Composable
private fun SettingRowDivider() {
    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
}

/**
 * A Material outlined card with a title. [description] says what the group is for, and
 * [granted] shows a chip with whether the phone already allows it.
 */
@Composable
private fun Section(
    title: String,
    highlighted: Boolean = false,
    description: String? = null,
    granted: Boolean? = null,
    content: @Composable () -> Unit,
) {
    val scheme = MaterialTheme.colorScheme
    androidx.compose.material3.OutlinedCard(
        modifier = Modifier.fillMaxWidth(),
        colors = androidx.compose.material3.CardDefaults.outlinedCardColors(
            containerColor = if (highlighted) scheme.secondaryContainer else scheme.surfaceContainerLow,
        ),
        border = BorderStroke(if (highlighted) 2.dp else 1.dp, if (highlighted) scheme.primary else scheme.outlineVariant),
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    title,
                    style = MaterialTheme.typography.titleMedium,
                    color = scheme.onSurface,
                    modifier = Modifier.weight(1f).semantics { heading() },
                )
                if (granted != null) StatusChip(granted)
            }
            if (description != null) {
                Text(description, style = MaterialTheme.typography.bodyMedium, color = scheme.onSurfaceVariant)
                SettingRowDivider()
            }
            content()
        }
    }
}

/** "Allowed" or "Not allowed yet", with an icon so colour isn't the only signal. */
@Composable
private fun StatusChip(granted: Boolean) {
    val scheme = MaterialTheme.colorScheme
    Surface(
        shape = RoundedCornerShape(8.dp),
        color = if (granted) scheme.tertiaryContainer else scheme.errorContainer,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        ) {
            androidx.compose.material3.Icon(
                androidx.compose.ui.res.painterResource(if (granted) R.drawable.ic_check_circle else R.drawable.ic_warning),
                contentDescription = null,
                tint = if (granted) scheme.onTertiaryContainer else scheme.onErrorContainer,
                modifier = Modifier.size(16.dp),
            )
            Text(
                stringResource(if (granted) R.string.status_allowed else R.string.status_missing),
                style = MaterialTheme.typography.labelMedium,
                color = if (granted) scheme.onTertiaryContainer else scheme.onErrorContainer,
            )
        }
    }
}

private class SettingsCategory(val id: String, val icon: Int, val title: String, val summary: String)

/** The sections of each category, in screen order (to scroll to one of them). */
private val CATEGORY_SECTIONS = mapOf(
    "devices" to listOf("devices"),
    "permissions" to listOf("perm_intro", "notifications", "control", "system", "background", "calls_contacts", "photos"),
    "features" to listOf("clipboard", "remote", "localsend"),
    "appearance" to listOf("appearance"),
    "video" to listOf("video"),
    "data" to listOf("data_storage"),
    "about" to listOf("about"),
)

private class SearchEntry(val category: String, val key: String, val title: String, val text: String)

/** Lower case, without accents: "Batería" matches "bateria". */
private fun normalize(s: String): String =
    java.text.Normalizer.normalize(s.lowercase(), java.text.Normalizer.Form.NFD).replace(Regex("\\p{Mn}+"), "")

/** Every section that search can find: its title, what it explains and a few extra words. */
@Composable
private fun settingsSearchIndex(): List<SearchEntry> {
    fun e(category: String, key: String, title: String, vararg more: String) =
        SearchEntry(category, key, title, (listOf(title) + more).joinToString(" "))
    return listOf(
        e("devices", "devices", stringResource(R.string.settings_devices), stringResource(R.string.action_unpair),
            stringResource(R.string.action_pair_new), stringResource(R.string.guide_away_title), stringResource(R.string.detailed_permissions), stringResource(R.string.profile_title),
            "pc computer ordenador vincular emparejar desvincular datos moviles mobile data away qr"),
        e("permissions", "notifications", stringResource(R.string.settings_notifications), stringResource(R.string.perm_desc_notifications)),
        e("permissions", "control", stringResource(R.string.settings_control), stringResource(R.string.perm_desc_control),
            "accesibilidad accessibility elevado elevated adb mouse raton teclado keyboard"),
        e("permissions", "system", stringResource(R.string.settings_system_title), stringResource(R.string.perm_desc_system),
            "no molestar do not disturb dnd volumen volume"),
        e("permissions", "background", stringResource(R.string.settings_background_title), stringResource(R.string.perm_desc_background),
            "bateria battery segundo plano background optimizacion"),
        e("permissions", "calls_contacts", stringResource(R.string.settings_calls_and_contacts), stringResource(R.string.perm_desc_calls),
            stringResource(R.string.sms_title), stringResource(R.string.calls_bluetooth_title), "sms mensajes messages bluetooth"),
        e("permissions", "photos", stringResource(R.string.settings_photos), stringResource(R.string.perm_desc_photos),
            "galeria gallery archivos files almacenamiento storage carpetas folders"),
        e("features", "clipboard", stringResource(R.string.settings_clipboard_title), stringResource(R.string.settings_clipboard_suggestions),
            "copiar pegar copy paste"),
        e("features", "remote", stringResource(R.string.settings_remote), stringResource(R.string.settings_touchpad_sensitivity),
            "touchpad panel tactil raton mouse sensibilidad"),
        e("features", "localsend", stringResource(R.string.settings_localsend_title), stringResource(R.string.settings_localsend_subtitle)),
        e("appearance", "appearance", stringResource(R.string.settings_appearance), stringResource(R.string.settings_theme),
            stringResource(R.string.settings_mode), "catppuccin tokyo night tema theme color oscuro dark claro light idioma language english espanol"),
        e("video", "video", stringResource(R.string.settings_video), stringResource(R.string.video_quality_high),
            "calidad quality 720p 1080p fps 60 pantalla screen camara camera webcam fluidez"),
        e("data", "data_storage", stringResource(R.string.settings_data_storage_title), stringResource(R.string.data_timeline_title),
            stringResource(R.string.data_clipboard_title), "borrar clear cache historial history"),
        e("about", "about", stringResource(R.string.settings_about), stringResource(R.string.guide_title),
            stringResource(R.string.guide_open_walkthrough), "version licencia license ayuda help"),
    )
}

/** A Material search field for the settings. */
@Composable
private fun SettingsSearchField(query: String, onQuery: (String) -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val hint = stringResource(R.string.settings_search_hint)
    androidx.compose.material3.TextField(
        value = query,
        onValueChange = onQuery,
        singleLine = true,
        placeholder = { Text(hint) },
        leadingIcon = {
            androidx.compose.material3.Icon(androidx.compose.ui.res.painterResource(R.drawable.ic_search), contentDescription = null)
        },
        trailingIcon = if (query.isNotEmpty()) {
            {
                androidx.compose.material3.IconButton(onClick = { onQuery("") }) {
                    androidx.compose.material3.Icon(
                        androidx.compose.ui.res.painterResource(R.drawable.ic_close),
                        contentDescription = stringResource(R.string.settings_search_clear),
                    )
                }
            }
        } else null,
        shape = RoundedCornerShape(28.dp),
        colors = androidx.compose.material3.TextFieldDefaults.colors(
            focusedContainerColor = scheme.surfaceContainerHigh,
            unfocusedContainerColor = scheme.surfaceContainerHigh,
            focusedIndicatorColor = androidx.compose.ui.graphics.Color.Transparent,
            unfocusedIndicatorColor = androidx.compose.ui.graphics.Color.Transparent,
        ),
        keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Search),
        modifier = Modifier
            .fillMaxWidth()
            .border(1.dp, scheme.outline, RoundedCornerShape(28.dp))
            .semantics { contentDescription = hint },
    )
}

@Composable
private fun CategoryIcon(icon: Int) {
    Surface(shape = CircleShape, color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.size(40.dp)) {
        androidx.compose.foundation.layout.Box(contentAlignment = Alignment.Center) {
            androidx.compose.material3.Icon(
                androidx.compose.ui.res.painterResource(icon),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSecondaryContainer,
                modifier = Modifier.size(22.dp),
            )
        }
    }
}

/** One category of the settings: icon, name, what is inside, and an arrow. */
@Composable
private fun CategoryRow(cat: SettingsCategory, warn: Boolean, onClick: () -> Unit) {
    androidx.compose.material3.ListItem(
        headlineContent = { Text(cat.title, style = MaterialTheme.typography.titleMedium) },
        supportingContent = {
            Text(cat.summary, color = if (warn) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
        },
        leadingContent = { CategoryIcon(cat.icon) },
        trailingContent = {
            androidx.compose.material3.Icon(
                androidx.compose.ui.res.painterResource(R.drawable.ic_chevron_right),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        },
        colors = androidx.compose.material3.ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
        modifier = Modifier.clickable(role = Role.Button, onClick = onClick),
    )
}

@Composable
private fun Label(text: String) {
    Text(text, style = MaterialTheme.typography.titleSmall)
}

@Composable
private fun Choice(options: List<Pair<String, String>>, selected: String, onSelect: (String) -> Unit) {
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
        options.forEachIndexed { index, (value, label) ->
            SegmentedButton(
                selected = value == selected,
                onClick = { onSelect(value) },
                shape = SegmentedButtonDefaults.itemShape(index, options.size),
            ) { Text(label) }
        }
    }
}

/** Broche's public repository: code, issues and releases. */
private const val SOURCE_URL = "https://github.com/RobertCJLA/broche"
