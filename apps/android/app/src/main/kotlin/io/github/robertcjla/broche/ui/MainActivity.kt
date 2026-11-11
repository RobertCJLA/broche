// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.ui

import android.Manifest
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.material3.NavigationBarItem
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import io.github.robertcjla.broche.BrocheApplication
import io.github.robertcjla.broche.R
import io.github.robertcjla.broche.core.Core
import io.github.robertcjla.broche.core.CoreState
import io.github.robertcjla.broche.core.CoreStatus
import io.github.robertcjla.broche.core.LocalNetwork
import io.github.robertcjla.broche.core.PairingState
import io.github.robertcjla.broche.mirror.MirrorService
import io.github.robertcjla.broche.service.ConnectionService
import io.github.robertcjla.broche.ui.deck.DeckScreen
import io.github.robertcjla.broche.ui.guide.FeaturesGuideScreen
import io.github.robertcjla.broche.ui.guide.GuideHighlightTarget
import io.github.robertcjla.broche.ui.home.HomeScreen
import io.github.robertcjla.broche.ui.home.PcFilesScreen
import io.github.robertcjla.broche.ui.pairing.PairingActions
import io.github.robertcjla.broche.ui.pairing.PairingScreen
import io.github.robertcjla.broche.ui.remote.RemoteMode
import io.github.robertcjla.broche.ui.remote.RemoteScreen
import io.github.robertcjla.broche.ui.settings.SettingsScreen
import io.github.robertcjla.broche.ui.theme.Appearance
import io.github.robertcjla.broche.ui.theme.LocalReducedMotion
import io.github.robertcjla.broche.ui.theme.BrocheTheme
import io.github.robertcjla.broche.ui.theme.liquidGlassCard
import io.github.robertcjla.broche.ui.walkthrough.WalkthroughScreen
import io.github.robertcjla.broche.ui.walkthrough.WalkthroughStep
import io.github.robertcjla.broche.ui.walkthrough.resolveStartupWalkthrough
import io.github.robertcjla.broche.ui.webcam.WebcamScreen
import io.github.robertcjla.broche.webcam.WebcamService
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    @Volatile
    private var activeRemotePcId: String? = null
    private var requestedRemotePc by mutableStateOf<String?>(null)
    private var requestedRemoteMode by mutableStateOf(RemoteMode.Touchpad)
    private var requestedDeckPc by mutableStateOf<String?>(null)

    /**
     * A pairing link that arrived from outside the app (a web page, another app, the system
     * camera). Pairing waits for the user to confirm: any app or site can send such a link,
     * and pairing grants that PC this phone's notifications and messages.
     */
    private var pendingPairingLink by mutableStateOf<String?>(null)
    private var requestedWebcamPc by mutableStateOf<String?>(null)
    private var requestedWebcamAutoStart by mutableStateOf(false)
    private var requestedWebcamHeight by mutableStateOf<Int?>(null)
    private var requestedWebcamCamera by mutableStateOf<String?>(null)

    private val permissions =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
            // The local network may have just been allowed: reconnect now.
            (application as BrocheApplication).core.refreshNotificationAccess()
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val core = (application as BrocheApplication).core
        val preferences = Preferences(this)

        // Notifications (transfers, calls, requests from PCs) and, on Android
        // 17, the local network, without which no PC can be reached.
        window.decorView.post {
            val wanted = buildList {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) add(Manifest.permission.POST_NOTIFICATIONS)
                if (LocalNetwork.needed()) add(LocalNetwork.PERMISSION)
            }.filter { checkSelfPermission(it) != android.content.pm.PackageManager.PERMISSION_GRANTED }
            if (wanted.isNotEmpty()) permissions.launch(wanted.toTypedArray())
        }
        if (savedInstanceState == null) {
            handlePairingLink(intent)
            handleRemoteIntent(intent)
            handleDeckIntent(intent)
            handleWebcamIntent(intent)
        }

        setContent {
            val appearance by preferences.appearance.collectAsStateWithLifecycle()
            val state by core.state.collectAsStateWithLifecycle()
            // Stay reachable in the background while a PC is paired.
            LaunchedEffect(state.devices.isNotEmpty(), state.status is CoreStatus.Ready) {
                if (state.devices.isNotEmpty()) {
                    ConnectionService.start(this@MainActivity)
                } else if (state.status is CoreStatus.Ready) {
                    ConnectionService.stop(this@MainActivity)
                }
            }
            BrocheTheme(
                appearance = appearance,
            ) {
                // Every screen sits on the theme's background, not the
                // window's (which can't follow the app's own light/dark choice).
                Surface(color = MaterialTheme.colorScheme.background) {
                    App(
                        core = core,
                        state = state,
                        appearance = appearance,
                        preferences = preferences,
                        requestedRemotePc = requestedRemotePc,
                        requestedRemoteMode = requestedRemoteMode,
                        onRemoteConsumed = { requestedRemotePc = null },
                        requestedDeckPc = requestedDeckPc,
                        onDeckConsumed = { requestedDeckPc = null },
                        requestedWebcamPc = requestedWebcamPc,
                        requestedWebcamAutoStart = requestedWebcamAutoStart,
                        requestedWebcamHeight = requestedWebcamHeight,
                        requestedWebcamCamera = requestedWebcamCamera,
                        onWebcamConsumed = {
                            requestedWebcamPc = null
                            requestedWebcamAutoStart = false
                            requestedWebcamHeight = null
                            requestedWebcamCamera = null
                        },
                        onActiveRemoteChanged = { activeRemotePcId = it },
                    )
                    pendingPairingLink?.let { link ->
                        val pcName = remember(link) {
                            android.net.Uri.parse(link).getQueryParameter("n")?.trim()?.takeIf { it.isNotEmpty() }
                        } ?: stringResource(R.string.pair_link_unknown_pc)
                        androidx.compose.material3.AlertDialog(
                            onDismissRequest = { pendingPairingLink = null },
                            title = { Text(stringResource(R.string.pair_link_title, pcName)) },
                            text = { Text(stringResource(R.string.pair_link_body)) },
                            confirmButton = {
                                androidx.compose.material3.TextButton(onClick = {
                                    pendingPairingLink = null
                                    core.joinWhenReady(link)
                                }) { Text(stringResource(R.string.pair_link_confirm)) }
                            },
                            dismissButton = {
                                androidx.compose.material3.TextButton(onClick = { pendingPairingLink = null }) {
                                    Text(stringResource(R.string.pair_link_cancel))
                                }
                            },
                        )
                    }
                }
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // Back from Settings, maybe with notification access changed.
        val app = application as BrocheApplication
        if (app.core.state.value.status is CoreStatus.Ready) {
            app.core.refreshNotificationAccess()
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handlePairingLink(intent)
        handleRemoteIntent(intent)
        handleDeckIntent(intent)
        handleWebcamIntent(intent)
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        val pcId = activeRemotePcId
        if (pcId != null) {
            when (keyCode) {
                KeyEvent.KEYCODE_VOLUME_DOWN -> {
                    if (event?.repeatCount == 0) {
                        (application as BrocheApplication).core.remoteSlide(pcId, "next")
                    }
                    return true
                }
                KeyEvent.KEYCODE_VOLUME_UP -> {
                    if (event?.repeatCount == 0) {
                        (application as BrocheApplication).core.remoteSlide(pcId, "previous")
                    }
                    return true
                }
            }
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent?): Boolean {
        if (activeRemotePcId != null &&
            (keyCode == KeyEvent.KEYCODE_VOLUME_DOWN || keyCode == KeyEvent.KEYCODE_VOLUME_UP)
        ) {
            return true
        }
        return super.onKeyUp(keyCode, event)
    }

    private fun handleRemoteIntent(intent: Intent?) {
        val pc = intent?.getStringExtra("remote_pc") ?: return
        val mode = intent.getStringExtra("remote_mode")
        requestedRemoteMode = when {
            mode.equals("presentation", ignoreCase = true) -> RemoteMode.Presentation
            mode.equals("air", ignoreCase = true) ||
                mode.equals("air_mouse", ignoreCase = true) ||
                mode.equals("airmouse", ignoreCase = true) -> RemoteMode.AirMouse
            else -> RemoteMode.Touchpad
        }
        requestedRemotePc = pc
    }

    private fun handleDeckIntent(intent: Intent?) {
        val pc = intent?.getStringExtra("deck_pc") ?: return
        requestedDeckPc = pc
    }

    private fun handleWebcamIntent(intent: Intent?) {
        val pc = intent?.getStringExtra("webcam_pc") ?: return
        // Auto-start only for our own request notification (see WebcamRequests).
        requestedWebcamAutoStart = intent.getBooleanExtra("webcam_auto_start", false) &&
            io.github.robertcjla.broche.webcam.WebcamRequests.consumeAutoStartToken(intent.getStringExtra("webcam_token"))
        requestedWebcamHeight = intent.getIntExtra("webcam_height", 0).takeIf { it > 0 }
        requestedWebcamCamera = intent.getStringExtra("webcam_camera")
        requestedWebcamPc = pc
    }

    /**
     * A PC's pairing QR code opened from another app (e.g. the camera).
     * Each link holds a one-time secret, so a link is used once: Android
     * re-delivers the original intent when the task is restored or reopened
     * from Recents, and that must not start a stale pairing.
     */
    private fun handlePairingLink(intent: Intent?) {
        val link = intent?.takeIf { it.action == Intent.ACTION_VIEW }?.dataString ?: return
        if (!link.startsWith("broche://pair?")) return
        if (intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY != 0) return
        if (!PairingLinks(this).consume(link)) return
        pendingPairingLink = link
    }
}

private enum class Tab { Home, Activity, Settings }

private sealed interface ScreenRoute {
    data object Starting : ScreenRoute
    data class Failed(val message: String) : ScreenRoute
    data object Walkthrough : ScreenRoute
    data object FeaturesGuide : ScreenRoute
    data object Pairing : ScreenRoute
    data class Webcam(val pcId: String) : ScreenRoute
    data class Deck(val pcId: String) : ScreenRoute
    data class Remote(val pcId: String) : ScreenRoute
    data class PcFiles(val pcId: String) : ScreenRoute
    data object Main : ScreenRoute
}

@Composable
private fun App(
    core: Core,
    state: CoreState,
    appearance: Appearance,
    preferences: Preferences,
    requestedRemotePc: String?,
    requestedRemoteMode: RemoteMode,
    onRemoteConsumed: () -> Unit,
    requestedDeckPc: String?,
    onDeckConsumed: () -> Unit,
    requestedWebcamPc: String?,
    requestedWebcamAutoStart: Boolean,
    requestedWebcamHeight: Int?,
    requestedWebcamCamera: String?,
    onWebcamConsumed: () -> Unit,
    onActiveRemoteChanged: (String?) -> Unit,
) {
    var tab by rememberSaveable { mutableStateOf(Tab.Home) }
    var pairing by rememberSaveable { mutableStateOf(false) }
    var manualWalkthroughOpen by rememberSaveable { mutableStateOf(false) }
    var featuresGuideOpen by rememberSaveable { mutableStateOf(false) }
    var pendingHighlightTarget by rememberSaveable { mutableStateOf<GuideHighlightTarget?>(null) }
    var remotePcId by rememberSaveable { mutableStateOf<String?>(null) }
    var remoteInitialMode by rememberSaveable { mutableStateOf(RemoteMode.Touchpad) }
    var deckPcId by rememberSaveable { mutableStateOf<String?>(null) }
    var webcamPcId by rememberSaveable { mutableStateOf<String?>(null) }
    var pcFilesPcId by rememberSaveable { mutableStateOf<String?>(null) }
    var webcamAutoStart by rememberSaveable { mutableStateOf(false) }
    var webcamInitialHeight by rememberSaveable { mutableStateOf<Int?>(null) }
    var webcamInitialCamera by rememberSaveable { mutableStateOf<String?>(null) }
    val walkthroughPersistence by preferences.walkthrough.collectAsStateWithLifecycle()
    val startupWalkthrough = remember(walkthroughPersistence, state.devices.isNotEmpty()) {
        resolveStartupWalkthrough(
            persistence = walkthroughPersistence,
            hasPairedDevices = state.devices.isNotEmpty(),
        )
    }
    LaunchedEffect(state.status, startupWalkthrough.markCompletedAutomatically) {
        if (state.status is CoreStatus.Ready && startupWalkthrough.markCompletedAutomatically) {
            preferences.updateWalkthrough { it.copy(completed = true) }
        }
    }
    val snackbar = remember { SnackbarHostState() }
    val undoLabel = stringResource(R.string.send_queue_undo)
    LaunchedEffect(Unit) { core.messages.collect { snackbar.showSnackbar(it) } }
    LaunchedEffect(undoLabel) {
        core.queuedNotices.collectLatest { notice ->
            val result = snackbar.showSnackbar(
                message = notice.message,
                actionLabel = undoLabel,
                duration = androidx.compose.material3.SnackbarDuration.Short,
            )
            if (result == androidx.compose.material3.SnackbarResult.ActionPerformed) {
                if (notice.queueId != null) {
                    core.cancelQueueItem(notice.queueId)
                } else if (notice.peerId != null) {
                    core.cancelAllQueueItems(notice.peerId)
                }
            }
        }
    }
    LaunchedEffect(Unit) {
        core.clipSuggestions.collectLatest { notice ->
            val result = snackbar.showSnackbar(
                message = notice.message,
                actionLabel = notice.actionLabel,
                duration = androidx.compose.material3.SnackbarDuration.Short,
            )
            if (result == androidx.compose.material3.SnackbarResult.ActionPerformed) {
                core.runClipSuggestion(notice.suggestion)
            }
        }
    }

    LaunchedEffect(requestedRemotePc, state.devices) {
        val req = requestedRemotePc ?: return@LaunchedEffect
        val target = if (req == "first") state.devices.firstOrNull()?.id else state.device(req)?.id
        if (target != null) {
            deckPcId = null
            webcamPcId = null
            remoteInitialMode = requestedRemoteMode
            remotePcId = target
            onRemoteConsumed()
        }
    }

    LaunchedEffect(requestedDeckPc, state.devices) {
        val req = requestedDeckPc ?: return@LaunchedEffect
        val target = if (req == "first") state.devices.firstOrNull()?.id else state.device(req)?.id
        if (target != null) {
            remotePcId = null
            webcamPcId = null
            deckPcId = target
            onDeckConsumed()
        }
    }

    LaunchedEffect(requestedWebcamPc, state.devices) {
        val req = requestedWebcamPc ?: return@LaunchedEffect
        val target = if (req == "first") state.devices.firstOrNull()?.id else state.device(req)?.id
        if (target != null) {
            remotePcId = null
            deckPcId = null
            webcamAutoStart = requestedWebcamAutoStart
            webcamInitialHeight = requestedWebcamHeight
            webcamInitialCamera = requestedWebcamCamera
            webcamPcId = target
            onWebcamConsumed()
        }
    }

    val remoteDevice = remotePcId?.let { state.device(it) }
    val deckDevice = deckPcId?.let { state.device(it) }
    val webcamDevice = webcamPcId?.let { state.device(it) }
    LaunchedEffect(remoteDevice?.id) {
        onActiveRemoteChanged(remoteDevice?.id)
    }

    val actions = remember(core) {
        object : PairingActions {
            override fun visible(open: Boolean) = core.setPairingScreenOpen(open)
            override fun join(link: String) = core.joinWithLink(link)
            override fun pairNearby(id: String) = core.pairNearby(id)
            override fun confirm(matches: Boolean) = core.confirmCode(matches)
            override fun reset() = core.resetPairing()
            override fun applyProfile(pcId: String, profile: io.github.robertcjla.broche.ffi.PermissionProfile) =
                core.applyPermissionProfile(pcId, profile)
        }
    }

    val showWalkthrough = manualWalkthroughOpen ||
        (state.status is CoreStatus.Ready && startupWalkthrough.showWalkthrough)

    val pcFilesDevice = pcFilesPcId?.let { state.device(it) }

    val currentRoute: ScreenRoute = when {
        state.status is CoreStatus.Starting -> ScreenRoute.Starting
        state.status is CoreStatus.Failed -> ScreenRoute.Failed(state.status.message)
        showWalkthrough -> ScreenRoute.Walkthrough
        featuresGuideOpen -> ScreenRoute.FeaturesGuide
        pairing || state.pairing != PairingState.Idle -> ScreenRoute.Pairing
        webcamDevice != null -> ScreenRoute.Webcam(webcamDevice.id)
        deckDevice != null -> ScreenRoute.Deck(deckDevice.id)
        remoteDevice != null -> ScreenRoute.Remote(remoteDevice.id)
        pcFilesDevice != null -> ScreenRoute.PcFiles(pcFilesDevice.id)
        else -> ScreenRoute.Main
    }

    val reducedMotion = LocalReducedMotion.current
    AnimatedContent(
        targetState = currentRoute,
        transitionSpec = {
            if (reducedMotion || initialState is ScreenRoute.Starting) {
                EnterTransition.None togetherWith ExitTransition.None
            } else {
                val enteringSubScreen = targetState !is ScreenRoute.Main && initialState is ScreenRoute.Main
                val returningToMain = targetState is ScreenRoute.Main && initialState !is ScreenRoute.Main
                val spatialSpring = androidx.compose.animation.core.spring<androidx.compose.ui.unit.IntOffset>(
                    dampingRatio = androidx.compose.animation.core.Spring.DampingRatioNoBouncy,
                    stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow,
                )
                val scaleSpring = androidx.compose.animation.core.spring<Float>(
                    dampingRatio = androidx.compose.animation.core.Spring.DampingRatioNoBouncy,
                    stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow,
                )
                when {
                    enteringSubScreen -> (
                        fadeIn(tween(210, easing = FastOutSlowInEasing)) +
                            slideInHorizontally(animationSpec = spatialSpring) { (it * 0.08f).toInt() } +
                            scaleIn(initialScale = 0.95f, animationSpec = scaleSpring)
                        ) togetherWith (
                        fadeOut(tween(150)) +
                            slideOutHorizontally(animationSpec = spatialSpring) { -(it * 0.04f).toInt() } +
                            scaleOut(targetScale = 0.97f, animationSpec = scaleSpring)
                        )
                    returningToMain -> (
                        fadeIn(tween(200, easing = FastOutSlowInEasing)) +
                            slideInHorizontally(animationSpec = spatialSpring) { -(it * 0.04f).toInt() } +
                            scaleIn(initialScale = 0.97f, animationSpec = scaleSpring)
                        ) togetherWith (
                        fadeOut(tween(150)) +
                            slideOutHorizontally(animationSpec = spatialSpring) { (it * 0.08f).toInt() } +
                            scaleOut(targetScale = 0.95f, animationSpec = scaleSpring)
                        )
                    else -> fadeIn(tween(200)) togetherWith fadeOut(tween(150))
                }
            }
        },
        label = "screen_route",
    ) { route ->
        when (route) {
            ScreenRoute.Starting -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                if (reducedMotion) {
                    CircularProgressIndicator(progress = { 0.75f })
                } else {
                    CircularProgressIndicator()
                }
            }
            is ScreenRoute.Failed -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text(stringResource(R.string.core_failed, route.message))
            }
            ScreenRoute.Walkthrough -> {
                BackHandler(
                    enabled = manualWalkthroughOpen && walkthroughPersistence.currentStep.previous == null,
                ) {
                    manualWalkthroughOpen = false
                }
                Scaffold(
                    snackbarHost = { SnackbarHost(snackbar) },
                ) { padding ->
                    WalkthroughScreen(
                        state = state,
                        step = walkthroughPersistence.currentStep,
                        pairingActions = actions,
                        onStepChange = { nextStep ->
                            preferences.updateWalkthrough { it.goToStep(nextStep) }
                        },
                        onSkip = {
                            core.resetPairing()
                            pairing = false
                            manualWalkthroughOpen = false
                            preferences.updateWalkthrough { it.copy(completed = true) }
                        },
                        onFinishToHome = {
                            core.resetPairing()
                            pairing = false
                            manualWalkthroughOpen = false
                            tab = Tab.Home
                            preferences.updateWalkthrough {
                                it.copy(completed = true, currentStepIndex = WalkthroughStep.Done.index)
                            }
                        },
                        onOpenFeaturesGuide = {
                            core.resetPairing()
                            pairing = false
                            manualWalkthroughOpen = false
                            featuresGuideOpen = true
                            preferences.updateWalkthrough {
                                it.copy(completed = true, currentStepIndex = WalkthroughStep.Done.index)
                            }
                        },
                        onAccessChanged = core::refreshNotificationAccess,
                        modifier = Modifier.fillMaxSize().padding(padding),
                    )
                }
            }
            ScreenRoute.FeaturesGuide -> {
                BackHandler { featuresGuideOpen = false }
                Scaffold(
                    snackbarHost = { SnackbarHost(snackbar) },
                ) { padding ->
                    FeaturesGuideScreen(
                        state = state,
                        onBack = { featuresGuideOpen = false },
                        onShowMe = { target ->
                            core.resetPairing()
                            pairing = false
                            featuresGuideOpen = false
                            pendingHighlightTarget = target
                            tab = when (target) {
                                GuideHighlightTarget.HOME_PAIR_PC,
                                GuideHighlightTarget.HOME_PC_CARD,
                                GuideHighlightTarget.HOME_PC_AUDIO,
                                GuideHighlightTarget.HOME_SETUP_PERMISSIONS -> Tab.Home
                                GuideHighlightTarget.HOME_CLIPBOARD_HISTORY,
                                GuideHighlightTarget.HOME_TIMELINE,
                                GuideHighlightTarget.HOME_SEND_QUEUE,
                                GuideHighlightTarget.HOME_LOCALSEND -> Tab.Activity
                                GuideHighlightTarget.SETTINGS_DEVICES,
                                GuideHighlightTarget.SETTINGS_NOTIFICATIONS,
                                GuideHighlightTarget.SETTINGS_CONTROL,
                                GuideHighlightTarget.SETTINGS_BACKGROUND,
                                GuideHighlightTarget.SETTINGS_CALLS_CONTACTS,
                                GuideHighlightTarget.SETTINGS_PHOTOS_STORAGE,
                                GuideHighlightTarget.SETTINGS_CLIPBOARD,
                                GuideHighlightTarget.SETTINGS_LOCALSEND,
                                GuideHighlightTarget.SETTINGS_DATA_STORAGE -> Tab.Settings
                            }
                        },
                        onOpenWalkthrough = {
                            core.resetPairing()
                            pairing = false
                            featuresGuideOpen = false
                            preferences.updateWalkthrough { it.goToStep(WalkthroughStep.Welcome) }
                            manualWalkthroughOpen = true
                        },
                        onPairNew = {
                            featuresGuideOpen = false
                            pairing = true
                        },
                        onAccessChanged = core::refreshNotificationAccess,
                        modifier = Modifier.fillMaxSize().padding(padding),
                    )
                }
            }
            ScreenRoute.Pairing -> {
                BackHandler(enabled = pairing || state.devices.isNotEmpty()) { core.resetPairing(); pairing = false }
                PairingScreen(state, actions, cancellable = pairing || state.devices.isNotEmpty()) { pairing = false }
            }
            is ScreenRoute.Webcam -> {
                val dev = state.device(route.pcId) ?: return@AnimatedContent
                BackHandler { webcamPcId = null }
                Scaffold(
                    snackbarHost = { SnackbarHost(snackbar) },
                ) { padding ->
                    WebcamScreen(
                        device = dev,
                        autoStart = webcamAutoStart,
                        initialHeight = webcamInitialHeight,
                        initialCamera = webcamInitialCamera,
                        onAutoStartConsumed = {
                            webcamAutoStart = false
                            webcamInitialHeight = null
                            webcamInitialCamera = null
                        },
                        onAccessChanged = core::refreshNotificationAccess,
                        onBack = { webcamPcId = null },
                        modifier = Modifier.fillMaxSize().padding(padding),
                    )
                }
            }
            is ScreenRoute.Deck -> {
                val dev = state.device(route.pcId) ?: return@AnimatedContent
                BackHandler { deckPcId = null }
                Scaffold(
                    snackbarHost = { SnackbarHost(snackbar) },
                ) { padding ->
                    DeckScreen(
                        device = dev,
                        core = core,
                        onBack = { deckPcId = null },
                        modifier = Modifier.fillMaxSize().padding(padding),
                    )
                }
            }
            is ScreenRoute.Remote -> {
                val dev = state.device(route.pcId) ?: return@AnimatedContent
                BackHandler { remotePcId = null }
                val sensitivity by preferences.touchpadSensitivity.collectAsStateWithLifecycle()
                Scaffold(
                    contentWindowInsets = androidx.compose.foundation.layout.WindowInsets(0, 0, 0, 0),
                    snackbarHost = { SnackbarHost(snackbar) },
                ) { padding ->
                    RemoteScreen(
                        device = dev,
                        core = core,
                        sensitivity = sensitivity,
                        initialMode = remoteInitialMode,
                        onBack = { remotePcId = null },
                        modifier = Modifier.fillMaxSize().padding(padding),
                    )
                }
            }
            is ScreenRoute.PcFiles -> {
                val dev = state.device(route.pcId) ?: return@AnimatedContent
                BackHandler { pcFilesPcId = null }
                Scaffold(
                    snackbarHost = { SnackbarHost(snackbar) },
                ) { padding ->
                    PcFilesScreen(
                        device = dev,
                        onListFiles = { path, onResult -> core.listPcFiles(dev.id, path, onResult) },
                        onDownloadFile = { remotePath, fileName, onSaved ->
                            core.downloadPcFile(dev.id, remotePath, fileName, onSaved)
                        },
                        onBack = { pcFilesPcId = null },
                        modifier = Modifier.fillMaxSize().padding(padding),
                    )
                }
            }
            ScreenRoute.Main -> Box(Modifier.fillMaxSize()) {
                val onlineCount by remember(state.devices) {
                    androidx.compose.runtime.derivedStateOf { state.devices.count { it.online } }
                }
                val pairedCount = state.devices.size
                // Material navigation: a bar at the bottom on phones, a rail on the side
                // when the window is wide (landscape, tablets, desktop mode).
                val wide = androidx.compose.ui.platform.LocalConfiguration.current.screenWidthDp >= 600
                Scaffold(
                    snackbarHost = { SnackbarHost(snackbar) },
                    bottomBar = {
                        if (!wide) {
                            MainNavigationBar(
                                selectedTab = tab,
                                onSelectTab = { tab = it },
                                onlineCount = onlineCount,
                                pairedCount = pairedCount,
                            )
                        }
                    },
                ) { padding ->
                    val context = LocalContext.current
                    val mirrorSession by MirrorService.session.collectAsStateWithLifecycle()
                    val webcamSession by WebcamService.session.collectAsStateWithLifecycle()
                    val touchpadSensitivity by preferences.touchpadSensitivity.collectAsStateWithLifecycle()
                    val suggestClipboardActions by preferences.suggestClipboardActions.collectAsStateWithLifecycle()
                    val autoClipboard by preferences.autoClipboard.collectAsStateWithLifecycle()
                    val saveableStateHolder = androidx.compose.runtime.saveable.rememberSaveableStateHolder()

                    // Only apply top scaffold padding here so LazyColumns in HomeScreen/SettingsScreen
                    // (which have bottom = 104.dp contentPadding) glide smoothly behind the floating glass dock.
                    // Tabs slide toward the side of the tab you picked, on a soft spring.
                    androidx.compose.foundation.layout.Row(
                        Modifier.fillMaxSize().padding(top = padding.calculateTopPadding(), bottom = padding.calculateBottomPadding()),
                    ) {
                    if (wide) {
                        MainNavigationRail(
                            selectedTab = tab,
                            onSelectTab = { tab = it },
                            onlineCount = onlineCount,
                            pairedCount = pairedCount,
                        )
                    }
                    AnimatedContent(
                        targetState = tab,
                        transitionSpec = {
                            if (reducedMotion) {
                                EnterTransition.None togetherWith ExitTransition.None
                            } else {
                                val forward = targetState.ordinal > initialState.ordinal
                                val slide = androidx.compose.animation.core.spring<androidx.compose.ui.unit.IntOffset>(
                                    dampingRatio = 0.82f,
                                    stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow,
                                )
                                (
                                    slideInHorizontally(slide) { w -> if (forward) w / 6 else -w / 6 } +
                                        fadeIn(tween(220, easing = FastOutSlowInEasing))
                                    ) togetherWith (
                                    slideOutHorizontally(slide) { w -> if (forward) -w / 10 else w / 10 } +
                                        fadeOut(tween(120))
                                    )
                            }
                        },
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxHeight(),
                        label = "main_tab",
                    ) { shownTab ->
                        when (shownTab) {
                            Tab.Home, Tab.Activity -> saveableStateHolder.SaveableStateProvider(if (shownTab == Tab.Home) "tab_home" else "tab_activity") {
                                HomeScreen(
                                    page = if (shownTab == Tab.Home) io.github.robertcjla.broche.ui.home.HomePage.Devices else io.github.robertcjla.broche.ui.home.HomePage.Activity,
                                    onOpenSettings = { tab = Tab.Settings },
                                    state = state,
                                    onRing = core::ring,
                                    onStopRinging = core::stopRinging,
                                    onPairNew = { pairing = true },
                                    onRefresh = core::refresh,
                                    onWake = core::wake,
                                    onRemote = { id ->
                                        remoteInitialMode = RemoteMode.Touchpad
                                        remotePcId = id
                                    },
                                    onDeck = { id ->
                                        deckPcId = id
                                    },
                                    onWebcam = { id ->
                                        webcamAutoStart = false
                                        webcamPcId = id
                                    },
                                    onSendFiles = core::sendFiles,
                                    onSendFolder = core::sendFolder,
                                    onAccessChanged = core::refreshNotificationAccess,
                                    onCancelTransfer = core::cancelTransfer,
                                    onPauseTransfer = core::pauseTransfer,
                                    onResumeTransfer = core::resumeTransfer,
                                    onAcceptTransfer = core::acceptTransfer,
                                    onCancelQueueItem = core::cancelQueueItem,
                                    onCancelAllQueueItems = { core.cancelAllQueueItems(null) },
                                    onRetryQueueItem = core::retryQueueItem,
                                    onClearFinishedQueueItems = { core.clearFinishedQueueItems(null) },
                                    onRefreshLocalSend = core::refreshLocalSend,
                                    onAllowStorage = { pcId -> core.setStorageEnabled(pcId, true) },
                                    onDismissStorageRequest = core::dismissStorageRequest,
                                    onAcceptWebcamRequest = { req ->
                                        core.clearWebcamRequest(req.pcId)
                                        webcamInitialHeight = req.height
                                        webcamInitialCamera = req.camera
                                        webcamAutoStart = true
                                        webcamPcId = req.pcId
                                    },
                                    onDismissWebcamRequest = { core.clearWebcamRequest() },
                                    onCopyClipboardHistory = core::copyClipboardHistory,
                                    onPinClipboardHistory = core::pinClipboardHistory,
                                    onDeleteClipboardHistory = core::deleteClipboardHistory,
                                    onClearClipboardHistory = core::clearClipboardHistory,
                                    loadClipboardHistoryImage = core::clipboardHistoryImage,
                                    queryTimelinePage = { kind, dev, search, offset ->
                                        core.queryTimelinePage(kind, dev, search, offset)
                                    },
                                    onDeleteTimelineEntry = core::deleteTimelineEntry,
                                    onClearTimeline = core::clearTimeline,
                                    onResendTimelineEntry = core::resendTimelineEntry,
                                    onSetPcAudio = core::setPcAudio,
                                    onStopKeyboardFromPc = core::stopKeyboardFromPc,
                                    onBrowsePcFiles = { pcFilesPcId = it },
                                    mirrorSession = mirrorSession,
                                    webcamSession = webcamSession,
                                    onStopMirror = { MirrorService.stopCurrent() },
                                    onOpenWebcam = { pcId ->
                                        webcamAutoStart = false
                                        webcamPcId = pcId
                                    },
                                    onStopWebcam = { WebcamService.stop(context) },
                                    showWhatsNewGuideBanner = startupWalkthrough.showWhatsNewGuideBanner,
                                    onDismissWhatsNewGuide = {
                                        preferences.updateWalkthrough { it.dismissWhatsNewGuide() }
                                    },
                                    onOpenFeaturesGuide = {
                                        preferences.updateWalkthrough { it.dismissWhatsNewGuide() }
                                        featuresGuideOpen = true
                                    },
                                    onOpenWalkthrough = {
                                        preferences.updateWalkthrough { it.goToStep(WalkthroughStep.Welcome) }
                                        manualWalkthroughOpen = true
                                    },
                                    highlightTarget = pendingHighlightTarget,
                                    onHighlightConsumed = { pendingHighlightTarget = null },
                                    modifier = Modifier.fillMaxSize(),
                                )
                            }
                            Tab.Settings -> saveableStateHolder.SaveableStateProvider("tab_settings") {
                                SettingsScreen(
                                    state = state,
                                    appearance = appearance,
                                    onAppearance = preferences::update,
                                    touchpadSensitivity = touchpadSensitivity,
                                    onTouchpadSensitivity = preferences::updateTouchpadSensitivity,
                                    onUnpair = core::unpair,
                                    onPairNew = { pairing = true },
                                    onAccessChanged = core::refreshNotificationAccess,
                                    onSetStorageEnabled = core::setStorageEnabled,
                                    onSetVideoQuality = core::setVideoQuality,
                                    onReadProfile = core::permissionProfile,
                                    onReadToggles = core::deviceToggles,
                                    onSetToggle = core::setDeviceToggle,
                                    onApplyProfile = core::applyPermissionProfile,
                                    onAddSafFolder = core::addSafFolder,
                                    onRemoveSafFolder = core::removeSafFolder,
                                    onSetClipboardHistoryEnabled = core::setClipboardHistoryEnabled,
                                    onClearClipboardHistory = core::clearClipboardHistory,
                                    suggestClipboardActions = suggestClipboardActions,
                                    onSetSuggestClipboardActions = preferences::updateSuggestClipboardActions,
                                    autoClipboard = autoClipboard,
                                    onSetAutoClipboard = preferences::updateAutoClipboard,
                                    onSetTimelineRetentionDays = core::setTimelineRetentionDays,
                                    onClearTimeline = core::clearTimeline,
                                    onClearMessageCache = core::clearMessageCache,
                                    onClearPhotoCache = core::clearPhotoThumbnailsCache,
                                    onClearReceivedFileHistory = core::clearReceivedFileHistory,
                                    onClearEverything = core::clearEverything,
                                    onToggleLocalSend = core::setLocalSendEnabled,
                                    onToggleAwayMode = core::setAwayMode,
                                    onRenamePhone = core::setPhoneName,
                                    onOpenWalkthrough = {
                                        preferences.updateWalkthrough { it.goToStep(WalkthroughStep.Welcome) }
                                        manualWalkthroughOpen = true
                                    },
                                    onOpenFeaturesGuide = {
                                        featuresGuideOpen = true
                                    },
                                    highlightTarget = pendingHighlightTarget,
                                    onHighlightConsumed = { pendingHighlightTarget = null },
                                    modifier = Modifier.fillMaxSize(),
                                )
                            }
                        }
                    }
                    }
                }
            }
        }
    }
}

private data class NavDestination(val tab: Tab, val icon: Int, val label: Int)

private val NAV_DESTINATIONS = listOf(
    NavDestination(Tab.Home, R.drawable.ic_home, R.string.home_title),
    NavDestination(Tab.Activity, R.drawable.ic_history, R.string.nav_activity),
    NavDestination(Tab.Settings, R.drawable.ic_settings, R.string.settings_title),
)

/** A dot on Home: connected (primary) or paired but offline (outline). */
@Composable
private fun NavIcon(dest: NavDestination, onlineCount: Int, pairedCount: Int) {
    val scheme = MaterialTheme.colorScheme
    val dot = when {
        dest.tab != Tab.Home -> null
        onlineCount > 0 -> scheme.primary
        pairedCount > 0 -> scheme.outline
        else -> null
    }
    androidx.compose.material3.BadgedBox(badge = {
        if (dot != null) androidx.compose.material3.Badge(containerColor = dot)
    }) {
        Icon(painterResource(dest.icon), contentDescription = null)
    }
}

@Composable
private fun MainNavigationBar(selectedTab: Tab, onSelectTab: (Tab) -> Unit, onlineCount: Int, pairedCount: Int) {
    androidx.compose.material3.NavigationBar {
        NAV_DESTINATIONS.forEach { dest ->
            NavigationBarItem(
                selected = selectedTab == dest.tab,
                onClick = { onSelectTab(dest.tab) },
                icon = { NavIcon(dest, onlineCount, pairedCount) },
                label = { Text(stringResource(dest.label)) },
            )
        }
    }
}

@Composable
private fun MainNavigationRail(selectedTab: Tab, onSelectTab: (Tab) -> Unit, onlineCount: Int, pairedCount: Int) {
    androidx.compose.material3.NavigationRail(
        modifier = Modifier.windowInsetsPadding(androidx.compose.foundation.layout.WindowInsets.displayCutout),
    ) {
        androidx.compose.foundation.layout.Spacer(Modifier.weight(1f))
        NAV_DESTINATIONS.forEach { dest ->
            androidx.compose.material3.NavigationRailItem(
                selected = selectedTab == dest.tab,
                onClick = { onSelectTab(dest.tab) },
                icon = { NavIcon(dest, onlineCount, pairedCount) },
                label = { Text(stringResource(dest.label)) },
            )
        }
        androidx.compose.foundation.layout.Spacer(Modifier.weight(1f))
    }
}
