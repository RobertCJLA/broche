// SPDX-License-Identifier: GPL-3.0-or-later
//! Broche for Linux: a graphical app on top of `broche-core`. It keeps a
//! single node online for the whole session, so the phone sees one steady link.

mod apps;
mod bt_calls;
mod bt_music;
mod clipwatch;
mod deck;
mod deps;
mod doctor;
mod history;
mod i18n;
mod island;
mod kit;
mod laser;
mod mirror;
mod palette;
mod pc_media;
mod phone_notify;
mod platform;
mod resources;
mod settings;
mod shelf;
mod shell;
mod theme;
mod tray;
mod welcome;
mod xdnd;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use broche_core::features::PermissionProfile;
use broche_core::{
    CallCommand, CallLogEntry, CallState, Contact, DeviceId, DeviceInfo, DeviceKind, LinkState, MediaAction,
    MirrorInput, MirrorStart, Node, NodeConfig, NodeEvent, Notification, PairingEvent, PhoneToggleValue,
    PhoneToggles, PhotoAlbum, PhotoItem, PhotoThumb, SmsMessage, SmsThread, StorageEntry, TransferState,
};
use eframe::egui;
use i18n::{Lang, t};
use platform::LinuxPlatform;
use tokio::{
    runtime::Runtime,
    sync::{broadcast::error::RecvError, mpsc},
    task::JoinHandle,
};

#[derive(Clone, Debug)]
#[allow(dead_code)] // kind and wake are kept for the PC actions that may come back
struct Device {
    name: String,
    /// "Pixel 7 Pro · Android 16", or just the system when the model isn't known.
    model: String,
    link: LinkState,
    kind: String,
    state: String,
    id: String,
    wake: bool,
}

/// What a file chooser was opened for.
#[derive(Debug, Clone)]
pub enum Pick {
    /// Send the chosen files to this phone (they land in its Download/Broche).
    Send(DeviceId),
    /// Copy the chosen files into this folder of the phone's storage.
    Upload(DeviceId, String),
    /// The folder of this PC that phones may browse.
    SharedFolder,
}

#[derive(Debug)]
pub enum Msg {
    /// The watch of that run stopped the phone's screen because nobody was looking: why.
    MirrorCut(u64, String),
    /// A phone notification was clicked on the desktop: show Broche's Notifications page.
    OpenNotifications,
    /// A transfer started, moved on or ended (the island shows the ones under way).
    Transfer(Box<broche_core::Transfer>),
    DownloadProgress(usize, u64, u64),
    DownloadFinished(usize, Result<(), String>),
    UploadFailed(DeviceId, String, String),
    /// Open or close the shelf (tray, palette, Home).
    ToggleShelf,
    /// The phone's apps for windows of their own, or why it couldn't list them.
    PhoneApps(DeviceId, Result<Vec<broche_core::PhoneApp>, String>),
    /// The phone chose a newer video quality.
    VideoQuality(broche_core::VideoQuality),
    /// The desktop's light/dark preference, from the Settings portal.
    SystemDark(bool),
    /// The connection check's findings.
    Doctor(Vec<doctor::Finding>),
    /// The core's history (timeline) changed: re-read the History page.
    HistoryChanged,
    /// Whether the desktop lets things move (Settings portal `reduced-motion`).
    Motion(bool),
    /// The Background portal refused or failed to turn autostart to this (Flatpak).
    AutostartFailed(bool, String),
    /// A phone was just paired: ask which permission profile it gets.
    AskProfile(DeviceId, String),
    Picked(Pick, Vec<PathBuf>),
    PhoneBattery(String, broche_core::Battery),
    PhoneMedia(String, Vec<broche_core::MediaPlayer>),
    StorageError(String),
    StorageRefresh(String),
    TrayAction(i32),
    OpenReceivedFolder,
    Quit,
    ShowWindow,
    Refresh,
    Line(String),
    PairLink(String),
    PairEnded,
    Sas(String),
    Clipboard(String),
    /// An image the phone copied to this PC's clipboard.
    ClipboardImage(egui::ColorImage),
    /// Text just copied on this PC (`clipwatch`).
    LocalCopy(String),
    SpeakerChanged(bool),
    AudioChanged,
    Threads(Vec<SmsThread>),
    SmsMessages(String, Vec<SmsMessage>),
    SmsDirty,
    CallLog(Vec<CallLogEntry>),
    Contacts(Vec<Contact>),
    Albums(Vec<PhotoAlbum>),
    PhotoItems(Vec<PhotoItem>),
    Thumbs(Vec<PhotoThumb>),
    Notif(Box<Notification>),
    NotifReset(Vec<Notification>),
    NotifGone(String),
    Call(CallState),
    Ringing(bool),
    /// The phone that asked this PC to ring (`None` when it stopped).
    RingFrom(Option<DeviceId>),
    PcMirrorAsk(DeviceId),
    Received(String),
    Captured(PathBuf),
    StorageList(String, Vec<StorageEntry>),
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum Tab {
    Devices,
    Screen,
    Messages,
    Calls,
    Notifications,
    Photos,
    Camera,
    Files,
    History,
    Apps,
    Pair,
    Perms,
    Access,
    About,
}

fn describe_link(link: &LinkState) -> String {
    match link {
        LinkState::Online { path, rtt_ms } => format!("online ({path:?}, {rtt_ms} ms)"),
        LinkState::Connecting => "connecting".into(),
        LinkState::Offline { .. } => "offline".into(),
    }
}

fn capabilities() -> Vec<String> {
    use broche_core as c;
    vec![
        "pc.power".into(),
        c::PC_WAKE.into(),
        "media.control".into(),
        c::DECK_ACTIONS.into(),
        c::PHOTOS_SHOW.into(),
        c::CALLS_SHOW.into(),
        c::CONTACTS_SHOW.into(),
        c::SMS_SHOW.into(),
        c::MIRROR_VIEW.into(),
        c::MIRROR_LISTEN.into(),
        c::PC_AUDIO.into(),
        c::TOGGLES_SHOW.into(),
        c::MIRROR_INPUT.into(),
        c::INPUT_INJECT.into(),
        c::MIRROR_CAPTURE.into(),
        c::WEBCAM_VIRTUAL.into(),
        c::CAMERA_CONTINUITY.into(),
        c::STORAGE_READ.into(),
        c::STORAGE_WRITE.into(),
    ]
}

/// `text` starting with a capital letter (the core's errors start in lower case).
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// Where files from the phone land: `~/Broche/Recibidos`.
fn received_dir() -> PathBuf {
    broche_dir().join("Recibidos")
}

/// `~/Broche`: what the phone may browse (by default) and, in `Recibidos`, what it sent.
fn broche_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("Broche")
}

/// The folder phones may browse when the user allows it: one of its own, not all of Home.
fn shared_dir() -> PathBuf {
    broche_dir()
}

/// How long the phone's screen may go unseen (another page) before it stops by itself.
const UNSEEN: std::time::Duration = std::time::Duration::from_secs(180);
/// How long Broche may stay minimized while showing it.
const HIDDEN: std::time::Duration = std::time::Duration::from_secs(30);
/// How often that is checked.
const WATCH: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Default)]
struct Downloads {
    items: Vec<Download>,
    generation: u64,
}

#[derive(Default)]
struct FileTransfers {
    items: Vec<broche_core::Transfer>,
    failed_uploads: std::collections::HashSet<(DeviceId, String)>,
}

impl FileTransfers {
    fn accept_report(&mut self, transfer: &broche_core::Transfer) -> bool {
        if transfer.direction != broche_core::Direction::Outgoing {
            return true;
        }
        let key = (transfer.device, transfer.id.clone());
        if transfer.state.is_finished() {
            self.failed_uploads.remove(&key);
            true
        } else {
            !self.failed_uploads.contains(&key)
        }
    }

    fn fail_upload(&mut self, peer: DeviceId, id: &str, error: String) -> Option<broche_core::Transfer> {
        let mut transfer = self
            .items
            .iter()
            .find(|tr| {
                tr.device == peer
                    && tr.id == id
                    && tr.direction == broche_core::Direction::Outgoing
                    && !tr.state.is_finished()
            })?
            .clone();
        // The local result can arrive before the core's earlier Waiting/Running events.
        self.failed_uploads.insert((peer, id.to_owned()));
        transfer.state = TransferState::Failed(broche_core::TransferFailure::Other(error));
        Some(transfer)
    }

    fn update(&mut self, transfer: &broche_core::Transfer) {
        if let Some(item) = self.items.iter_mut().find(|item| {
            item.id == transfer.id && item.device == transfer.device && item.direction == transfer.direction
        }) {
            *item = transfer.clone();
        } else {
            self.items.push(transfer.clone());
        }
    }
}

struct Download {
    name: String,
    device: DeviceId,
    total: u64,
    done: u64,
    started: bool,
    result: Option<Result<(), String>>,
}

impl Downloads {
    fn add(&mut self, name: String, total: u64, device: DeviceId) -> usize {
        if self.finished() {
            self.items.clear();
            self.generation = self.generation.wrapping_add(1);
        }
        let id = self.items.len();
        self.items.push(Download { name, device, total, done: 0, started: false, result: None });
        id
    }

    fn progress(&mut self, id: usize, done: u64, total: u64) {
        if let Some(item) = self.items.get_mut(id) {
            item.started = true;
            item.total = total;
            item.done = done.min(total);
        }
    }

    fn finish(&mut self, id: usize, result: Result<(), String>) {
        if let Some(item) = self.items.get_mut(id) {
            if result.is_ok() {
                item.done = item.total;
            }
            item.result = Some(result);
        }
    }

    fn finished(&self) -> bool {
        self.items.iter().all(|item| item.result.is_some())
    }

    fn completed(&self) -> usize {
        self.items.iter().filter(|item| matches!(item.result, Some(Ok(())))).count()
    }

    fn failed(&self) -> usize {
        self.items.iter().filter(|item| matches!(item.result, Some(Err(_)))).count()
    }

    fn bytes(&self) -> (u64, u64) {
        self.items.iter().fold((0u64, 0u64), |(done, total), item| {
            (done.saturating_add(item.done), total.saturating_add(item.total))
        })
    }

    fn fraction(&self) -> f32 {
        let (done, total) = self.bytes();
        if total > 0 {
            (done as f64 / total as f64).clamp(0.0, 1.0) as f32
        } else if !self.items.is_empty() && self.finished() && self.failed() == 0 {
            1.0
        } else {
            0.0
        }
    }
}

/// What the phone's own screen does while it is shown on this PC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MirrorScreen {
    AsUsual,
    KeepOn,
    Off,
}

/// The name the system gives this PC: GNOME's "Device name" (`PRETTY_HOSTNAME`) if set,
/// else the host name ("fedora" on many Fedora installs).
pub(crate) fn default_device_name() -> String {
    std::fs::read_to_string("/etc/machine-info")
        .ok()
        .and_then(|t| pretty_hostname(&t))
        .or_else(|| hostname::get().ok().and_then(|h| h.into_string().ok()))
        .map(|n| broche_core::clean_device_name(&n))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Broche".into())
}

/// `PRETTY_HOSTNAME` from `/etc/machine-info` (shell-style `KEY=value`, maybe quoted).
fn pretty_hostname(machine_info: &str) -> Option<String> {
    let v = machine_info.lines().find_map(|l| l.trim().strip_prefix("PRETTY_HOSTNAME="))?.trim();
    let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v);
    Some(v.replace("\\\"", "\"")).filter(|v| !v.trim().is_empty())
}

/// The name phones see: the one chosen in Preferences, or the system's.
pub(crate) fn device_name() -> String {
    let saved = broche_core::clean_device_name(&settings::Settings::load().device_name);
    if saved.is_empty() { default_device_name() } else { saved }
}

fn data_dir() -> Option<PathBuf> {
    match std::env::var_os("BROCHE_DATA_DIR") {
        Some(d) => Some(PathBuf::from(d)),
        None => Some(dirs::data_local_dir()?.join("broche")),
    }
}

async fn start_node(platform: Arc<LinuxPlatform>) -> Result<Node, String> {
    let data_dir = data_dir().ok_or("no local data directory")?;
    let device = DeviceInfo {
        name: device_name(),
        kind: DeviceKind::Desktop,
        os: std::env::consts::OS.into(),
        os_ver: String::new(),
        model: None,
        accent: None,
        screen: None,
        install_id: None,
    };
    let mut config = NodeConfig::new(data_dir, device, env!("CARGO_PKG_VERSION"));
    config.capabilities = capabilities();
    config.downloads_dir = Some(received_dir());
    // A phone plugged in with USB debugging pairs (and connects) over the cable. Needs a running
    // adb server; without one the watcher just retries quietly.
    configure_discovery(&mut config, std::env::var_os("BROCHE_SHOT").is_some());
    let saved = settings::Settings::load();
    config.video_quality = broche_core::VideoQuality { high: saved.experimental60, at: saved.video_at };
    let _ = std::fs::create_dir_all(received_dir());
    let node = Node::start(config, platform.clone()).await.map_err(|e| format!("failed to start: {e}"))?;
    let _ = platform.node.set(node.clone());
    let deck = deck::layout(&platform.deck.read().unwrap());
    // The node is running now: a missing layout must not make the caller start a second one.
    if let Err(e) = node.set_deck_layout(deck).await {
        eprintln!("broche: PC shortcuts layout: {e}");
    }
    // shortcut: read once at start; an adapter plugged in later counts from the next start.
    let macs = platform::wake_macs(std::path::Path::new("/sys/class/net"));
    node.set_wake_info(broche_core::PcWakeInfo { macs, broadcasts: vec![] }).await;
    tokio::spawn(pc_media::watch(node.clone()));
    tokio::spawn(bt_calls::watch());
    tokio::spawn(bt_music::watch());
    Ok(node)
}

fn configure_discovery(config: &mut NodeConfig, preview: bool) {
    // Screenshots keep LAN discovery (the simulated phone is found that way) but never
    // reach a real phone over USB with a throwaway identity.
    config.adb_server = (!preview).then_some(broche_core::usb::DEFAULT_ADB_ADDR);
}

/// Peer, when its permissions were read, and the list (re-read at most every 2 s).
type PermCache = (DeviceId, std::time::Instant, Vec<(&'static str, bool)>);

struct App {
    rt: Runtime,
    node: Option<Node>,
    platform: Arc<LinuxPlatform>,
    tx: mpsc::UnboundedSender<Msg>,
    rx: mpsc::UnboundedReceiver<Msg>,
    lang: Lang,
    tab: Tab,
    devices: Vec<Device>,
    selected: Option<String>,
    log: Vec<String>,
    // pairing
    pair_link: Option<String>,
    pair_task: Option<JoinHandle<()>>,
    sas: Option<String>,
    // device page
    clip_text: String,
    pc_volume: (u8, bool),
    speaker: bool,
    // screen
    mirror_tex: Option<egui::TextureHandle>,
    mirror_input: Option<mpsc::UnboundedSender<MirrorInput>>,
    mirror_wanted: bool,
    mirror_was_active: bool,
    mirror_sound: bool,
    /// The phone's own screen while it is shown here: as usual, kept on, or turned off (which
    /// also keeps the phone awake). One choice, so the two can't be asked for together.
    mirror_screen: MirrorScreen,
    mirror_text: String,
    touching: bool,
    /// "Phone only": the window shows just the phone's screen (`phone_window`).
    phone_window: bool,
    /// The window's size before "phone only", given back when it ends.
    full_size: Option<egui::Vec2>,
    // messages
    threads: Vec<SmsThread>,
    open_thread: Option<String>,
    sms_messages: Vec<SmsMessage>,
    sms_to: String,
    sms_body: String,
    // calls & contacts
    calls: Vec<CallLogEntry>,
    contacts: Vec<Contact>,
    contact_query: String,
    dial: String,
    call: Option<CallState>,
    // notifications
    notifications: Vec<Notification>,
    reply_text: HashMap<String, String>,
    // photos
    albums: Vec<PhotoAlbum>,
    album: Option<String>,
    photos: Vec<PhotoItem>,
    thumbs: HashMap<String, egui::TextureHandle>,
    // ring, screen requests, files
    logo: Option<egui::TextureHandle>,
    tray_state: Arc<std::sync::RwLock<tray::TrayState>>,
    tray_available: Arc<std::sync::atomic::AtomicBool>,
    quitting: bool,
    confirm_unpair: Option<DeviceId>,
    /// Text typed in the search field of Settings.
    settings_query: String,
    theme_pref: egui::ThemePreference,
    scheme: theme::Scheme,
    theme_applied: Option<(egui::ThemePreference, theme::Scheme, Option<bool>)>,
    /// The desktop's dark preference (Settings portal); `None` until known.
    system_dark: Option<bool>,
    zoom: f32,
    zoom_set: bool,
    close_quits: bool,
    /// Copies go to the phone on their own; the phone's arrive with a notification.
    clipboard_auto: bool,
    /// The welcome page being shown, if it is open.
    welcome_step: Option<u8>,
    /// What the "Name of this PC" field holds while it is edited.
    name_edit: String,
    /// The clipboard text last sent or received, so it isn't bounced back.
    last_clip: Option<String>,
    autostart_on: bool,
    start_minimized: bool,
    shot_dir: Option<PathBuf>,
    history: history::History,
    apps: apps::Apps,
    shelf: Option<Arc<shelf::Shelf>>,
    /// The connection check's last findings (`None`: not run, or running).
    doctor: Option<Vec<doctor::Finding>>,
    doctor_running: bool,
    palette: palette::CommandPalette,
    shot_step: usize,
    frame_no: u64,
    shot_since: std::time::Instant,
    autostart: Option<String>,
    last_check: std::time::Instant,
    snap_screen: mirror::Counters,
    snap_cam: mirror::Counters,
    cam_tex: Option<egui::TextureHandle>,
    cam_wanted: bool,
    cam_was_active: bool,
    /// Save the next webcam frame as a photo.
    cam_snap: bool,
    last_capture: Option<PathBuf>,
    ringing: bool,
    ring_from: Option<DeviceId>,
    mirror_ask: Option<DeviceId>,
    received: Vec<String>,
    storage_path: String,
    storage_entries: Vec<StorageEntry>,
    storage_loaded: bool,
    storage_loading: bool,
    storage_error: Option<String>,
    storage_list: bool,
    storage_hidden: bool,
    /// Files picked in the current phone folder, to download together.
    storage_selected: std::collections::BTreeSet<String>,
    downloads: Downloads,
    file_transfers: FileTransfers,
    island_transfers: island::Transfers,
    shared_edit: String,
    new_folder: String,
    new_folder_open: bool,
    photos_requested: bool,
    composing: bool,
    batteries: HashMap<String, broche_core::Battery>,
    /// What each phone is playing, newest first (from its media events).
    players: HashMap<String, Vec<broche_core::MediaPlayer>>,
    /// Album art of the phone's players by art key (the phone sends each picture once).
    art: HashMap<String, egui::TextureHandle>,
    perm_cache: Option<PermCache>,
    caps_cache: Option<(DeviceId, std::time::Instant, (bool, bool))>,
    experimental60: bool,
    /// The tip shown on Home (an index into `shell::TIPS`); None when hidden.
    tip: Option<usize>,
    /// Preference: no tips on Home.
    hide_tips: bool,
    /// Show the island (Preferences › Window).
    island_on: bool,
    /// Phone notifications shown on the desktop too.
    phone_notify: bool,
    bt_phone: String,
    bt_talk_on_answer: bool,
    island: Option<Arc<island::Island>>,
    /// The island's last activity and when it was seen, and how long it stays after (seconds).
    island_last: Option<(island::Activity, std::time::Instant)>,
    island_linger: u32,
    /// Running on X11 (XWayland); false only in the plain Wayland fallback.
    on_x11: bool,
    deck_editor: deck::Editor,
    /// What About lists as missing (looked up when About first opens).
    missing_programs: Option<Vec<&'static deps::Need>>,
    /// A phone just paired, and the profile picked for it (Basic until changed).
    profile_prompt: Option<(DeviceId, String, PermissionProfile)>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let saved = settings::Settings::load();
        phone_notify::set_enabled(saved.phone_notifications);
        let lang = match saved.lang.as_str() {
            "es" => Lang::Es,
            "en" => Lang::En,
            _ => i18n::detect(),
        };
        i18n::set(lang);
        let theme_pref = match saved.theme.as_str() {
            "light" => egui::ThemePreference::Light,
            "dark" => egui::ThemePreference::Dark,
            _ => egui::ThemePreference::System,
        };
        // Unset, or the whole Home folder (what older builds shared): use the dedicated folder.
        // A folder the user chose is kept. Each phone keeps its own storage permission.
        let shared_folder = match saved.shared_dir.as_str() {
            "" => shared_dir(),
            dir if dirs::home_dir().is_some_and(|h| h == Path::new(dir)) => shared_dir(),
            dir => PathBuf::from(dir),
        };
        if saved.shared_dir != shared_folder.to_string_lossy() {
            settings::Settings::update(|s| s.shared_dir = shared_folder.display().to_string());
        }
        let _ = std::fs::create_dir_all(&shared_folder);
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime");
        let (tx, rx) = mpsc::unbounded_channel();
        log_core_to(tx.clone());
        let shared = Arc::new(mirror::MirrorShared::default());
        *shared.ctx.lock().unwrap() = Some(cc.egui_ctx.clone());
        let cam = Arc::new(mirror::MirrorShared::default());
        *cam.ctx.lock().unwrap() = Some(cc.egui_ctx.clone());
        let platform =
            LinuxPlatform::new(rt.handle().clone(), tx.clone(), shared, cam, shared_folder.clone());
        let mut log = vec![];
        // Started with the session, the keyring and the portals may still be starting: retry
        // for up to 30 s (the window is minimized anyway) before giving up.
        let tries = if std::env::args().any(|a| a == "--minimized") { 10 } else { 1 };
        let mut node = None;
        for attempt in 1..=tries {
            match rt.block_on(start_node(platform.clone())) {
                Ok(n) => {
                    node = Some(n);
                    break;
                }
                Err(e) => {
                    eprintln!("broche: {e} (attempt {attempt} of {tries})");
                    if attempt == tries {
                        log.push(e);
                    } else {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                    }
                }
            }
        }
        if let Some(node) = &node {
            let mut events = node.events();
            let tx = tx.clone();
            let ctx = cc.egui_ctx.clone();
            rt.spawn(async move {
                loop {
                    let msg = match events.recv().await {
                        Ok(
                            NodeEvent::DeviceAdded(_)
                            | NodeEvent::DeviceRemoved(_)
                            | NodeEvent::LinkChanged { .. }
                            | NodeEvent::PhoneToggles { .. },
                        ) => Msg::Refresh,
                        Ok(NodeEvent::Battery { device, battery }) => {
                            Msg::PhoneBattery(device.to_string(), battery)
                        }
                        Ok(NodeEvent::VideoQuality(q)) => Msg::VideoQuality(q),
                        Ok(NodeEvent::Ring { device, on }) => Msg::RingFrom(on.then_some(device)),
                        // Also pairings the PC didn't start from its QR (a phone plugged in by USB).
                        Ok(NodeEvent::Pairing(PairingEvent::SasCode { code, .. })) => Msg::Sas(code),
                        Ok(NodeEvent::Pairing(PairingEvent::Paired(d))) => {
                            Msg::AskProfile(d.id, d.display_name)
                        }
                        Ok(NodeEvent::Pairing(PairingEvent::Failed(_))) => Msg::PairEnded,
                        Ok(NodeEvent::TimelineChanged) => Msg::HistoryChanged,
                        Ok(NodeEvent::MediaChanged { device, players }) => {
                            Msg::PhoneMedia(device.to_string(), players)
                        }
                        Ok(NodeEvent::NotificationPosted { device, notification }) => {
                            let copy = notification.clone();
                            tokio::spawn(async move { phone_notify::posted(device, &copy).await });
                            Msg::Notif(Box::new(notification))
                        }
                        Ok(NodeEvent::NotificationsReset { items, .. }) => Msg::NotifReset(items),
                        Ok(NodeEvent::NotificationRemoved { device, key }) => {
                            let gone = key.clone();
                            tokio::spawn(async move { phone_notify::removed(device, &gone).await });
                            Msg::NotifGone(key)
                        }
                        Ok(NodeEvent::Call { call, .. }) => Msg::Call(call),
                        Ok(NodeEvent::SmsChanged { .. }) => Msg::SmsDirty,
                        Ok(NodeEvent::PcMirrorRequested { device, .. }) => Msg::PcMirrorAsk(device),
                        Ok(NodeEvent::CameraCaptureReceived { saved_path: Some(path), .. }) => {
                            Msg::Captured(path)
                        }
                        Ok(NodeEvent::CameraCaptureCancelled { reason, .. }) => Msg::Line(format!(
                            "{}{}",
                            t("Camera capture cancelled"),
                            reason.map(|r| format!(": {r}")).unwrap_or_default()
                        )),
                        Ok(NodeEvent::Transfer(tr)) => {
                            // Every step goes to the island; the end also gets its usual message.
                            let _ = tx.send(Msg::Transfer(Box::new(tr.clone())));
                            ctx.request_repaint();
                            match tr.state {
                                TransferState::Done { saved }
                                    if tr.direction == broche_core::Direction::Incoming =>
                                {
                                    let paths: Vec<String> =
                                        saved.iter().map(|p| p.display().to_string()).collect();
                                    Msg::Received(paths.join(", "))
                                }
                                TransferState::Failed(_) => {
                                    Msg::Line(format!("{} {}", t("Transfer failed:"), tr.names.join(", ")))
                                }
                                _ => continue,
                            }
                        }
                        Ok(_) | Err(RecvError::Lagged(_)) => continue,
                        Err(RecvError::Closed) => break,
                    };
                    let _ = tx.send(msg);
                    ctx.request_repaint();
                }
            });
        }
        let mut app = Self {
            rt,
            node,
            platform,
            tx,
            rx,
            lang,
            tab: Tab::Devices,
            devices: vec![],
            selected: None,
            log,
            pair_link: None,
            pair_task: None,
            sas: None,
            clip_text: String::new(),
            pc_volume: platform::pc_volume(),
            speaker: false,
            mirror_tex: None,
            mirror_input: None,
            mirror_wanted: false,
            mirror_was_active: false,
            mirror_sound: false,
            mirror_screen: MirrorScreen::KeepOn,
            mirror_text: String::new(),
            touching: false,
            phone_window: false,
            full_size: None,
            threads: vec![],
            open_thread: None,
            sms_messages: vec![],
            sms_to: String::new(),
            sms_body: String::new(),
            calls: vec![],
            contacts: vec![],
            contact_query: String::new(),
            dial: String::new(),
            call: None,
            notifications: vec![],
            reply_text: HashMap::new(),
            albums: vec![],
            album: None,
            photos: vec![],
            thumbs: HashMap::new(),
            logo: None,
            tray_state: Arc::new(std::sync::RwLock::new(tray::TrayState::default())),
            tray_available: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            quitting: false,
            confirm_unpair: None,
            settings_query: String::new(),
            theme_pref,
            scheme: theme::Scheme::from_id(&saved.scheme),
            theme_applied: None,
            system_dark: None,
            zoom: saved.zoom(),
            zoom_set: false,
            close_quits: saved.close_quits,
            clipboard_auto: saved.clipboard_auto,
            // Testing aid: `BROCHE_WELCOME=<page>` opens it at that page (from 1).
            welcome_step: match std::env::var("BROCHE_WELCOME").ok().and_then(|v| v.parse::<u8>().ok()) {
                Some(page) => Some(page.clamp(1, welcome::STEPS) - 1),
                None => (!saved.welcome_done).then_some(0),
            },
            last_clip: None,
            name_edit: device_name(),
            autostart_on: settings::autostart_enabled(),
            start_minimized: std::env::args().any(|a| a == "--minimized"),
            shot_dir: std::env::var_os("BROCHE_SHOT").map(PathBuf::from),
            history: Default::default(),
            apps: Default::default(),
            shelf: None,
            doctor: None,
            doctor_running: false,
            palette: Default::default(),
            shot_step: 0,
            frame_no: 0,
            shot_since: std::time::Instant::now(),
            autostart: std::env::var("BROCHE_AUTOSTART").ok(),
            last_check: std::time::Instant::now(),
            snap_screen: Default::default(),
            snap_cam: Default::default(),
            cam_tex: None,
            cam_wanted: false,
            cam_was_active: false,
            cam_snap: false,
            last_capture: None,
            ringing: false,
            ring_from: None,
            mirror_ask: None,
            received: vec![],
            storage_path: String::new(),
            storage_entries: vec![],
            storage_loaded: false,
            storage_loading: false,
            storage_error: None,
            storage_list: saved.storage_list,
            storage_hidden: false,
            storage_selected: Default::default(),
            downloads: Downloads::default(),
            file_transfers: FileTransfers::default(),
            island_transfers: island::Transfers::default(),
            shared_edit: shared_folder.display().to_string(),
            new_folder: String::new(),
            new_folder_open: false,
            photos_requested: false,
            composing: false,
            batteries: HashMap::new(),
            players: HashMap::new(),
            art: HashMap::new(),
            perm_cache: None,
            caps_cache: None,
            experimental60: saved.experimental60,
            tip: (!saved.hide_tips).then(shell::random_tip),
            hide_tips: saved.hide_tips,
            island_on: saved.island,
            phone_notify: saved.phone_notifications,
            bt_phone: saved.bt_phone.clone(),
            bt_talk_on_answer: saved.bt_talk_on_answer,
            island: None,
            island_last: None,
            island_linger: saved.island_linger,
            on_x11: std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_some(),
            deck_editor: deck::Editor::default(),
            missing_programs: None,
            profile_prompt: None,
        };
        app.refresh();
        app.platform.publish_audio_state();
        app.platform.watch_volume();
        app.rt.spawn(tray::run(
            app.tx.clone(),
            cc.egui_ctx.clone(),
            app.tray_available.clone(),
            app.tray_state.clone(),
            app.node.clone(),
            app.platform.clone(),
        ));
        {
            let (tx, ctx) = (app.tx.clone(), cc.egui_ctx.clone());
            app.rt.spawn(phone_notify::listen(tx, ctx));
            // Under XWayland winit hears nothing of the desktop's light/dark switch, so "Follow
            // the system" asks the Settings portal, which GNOME and KDE keep up to date.
            let (tx, ctx) = (app.tx.clone(), cc.egui_ctx.clone());
            app.rt.spawn(async move {
                use ashpd::desktop::settings::{ColorScheme, Settings};
                use futures_util::StreamExt;
                let Ok(settings) = Settings::new().await else { return };
                let send = |c: ColorScheme| {
                    let _ = tx.send(Msg::SystemDark(c == ColorScheme::PreferDark));
                    ctx.request_repaint();
                };
                if let Ok(c) = settings.color_scheme().await {
                    send(c);
                }
                if let Ok(mut changes) = settings.receive_color_scheme_changed().await {
                    while let Some(c) = changes.next().await {
                        send(c);
                    }
                }
            });
        }
        {
            // Reduce animation (GNOME's accessibility setting, KDE's too): no sliding or fading.
            let (tx, ctx) = (app.tx.clone(), cc.egui_ctx.clone());
            app.rt.spawn(async move {
                use ashpd::desktop::settings::{ReducedMotion, Settings};
                use futures_util::StreamExt;
                let Ok(settings) = Settings::new().await else { return };
                let send = |m: ReducedMotion| {
                    let _ = tx.send(Msg::Motion(m != ReducedMotion::ReducedMotion));
                    ctx.request_repaint();
                };
                if let Ok(m) = settings.reduced_motion().await {
                    send(m);
                }
                if let Ok(mut changes) = settings.receive_reduced_motion_changed().await {
                    while let Some(m) = changes.next().await {
                        send(m);
                    }
                }
            });
        }
        if app.on_x11 {
            let (tx, ctx) = (app.tx.clone(), cc.egui_ctx.clone());
            clipwatch::spawn(move |text| {
                let _ = tx.send(Msg::LocalCopy(text));
                ctx.request_repaint();
            });
        }
        if let Some(node) = app.node.clone() {
            app.rt.spawn(resources::report(node.clone()));
            app.rt.spawn(platform::report_pc_battery(node));
        }
        app
    }

    fn refresh(&mut self) {
        let was_online = self
            .devices
            .iter()
            .any(|d| Some(&d.id) == self.selected.as_ref() && d.state.starts_with("online"));
        let Some(node) = &self.node else { return };
        let list = node.paired_devices().unwrap_or_default();
        let d: Vec<Device> = list
            .iter()
            .map(|p| Device {
                name: p.display_name.clone(),
                model: [
                    p.info.model.clone().unwrap_or_default(),
                    format!("{} {}", capitalized(&p.info.os), p.info.os_ver),
                ]
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
                link: p.link.clone(),
                kind: format!("{:?}", p.info.kind),
                state: describe_link(&p.link),
                id: p.id.to_string(),
                wake: p.can_wake,
            })
            .collect();
        if self.selected.as_ref().is_none_or(|s| !d.iter().any(|x| &x.id == s)) {
            self.selected = d.first().map(|x| x.id.clone());
        }
        self.devices = d;
        let online = self
            .devices
            .iter()
            .any(|d| Some(&d.id) == self.selected.as_ref() && d.state.starts_with("online"));
        if online && !was_online {
            self.storage_loaded = false;
            self.storage_error = None;
        }
    }

    fn peer(&self) -> Option<DeviceId> {
        self.selected.as_ref()?.parse().ok()
    }

    /// Runs `f` on the node in the background; its result goes to the activity log.
    fn act<F, Fut>(&mut self, label: &str, f: F)
    where
        F: FnOnce(Node) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<String, String>> + Send + 'static,
    {
        let Some(node) = self.node.clone() else {
            self.log.push(t("Not connected to the Broche core."));
            return;
        };
        self.log.push(format!("$ {label}"));
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let line = match f(node).await {
                Ok(s) => s,
                Err(e) => format!("{} {e}", t("Failed:")),
            };
            let _ = tx.send(Msg::Line(line));
        });
    }

    /// Runs `f` on the node in the background and sends what it returns to the UI.
    fn load<F, Fut>(&self, f: F)
    where
        F: FnOnce(Node) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Msg, String>> + Send + 'static,
    {
        let Some(node) = self.node.clone() else { return };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            match f(node).await {
                Ok(m) => {
                    let _ = tx.send(m);
                }
                Err(e) => {
                    let _ = tx.send(Msg::Line(format!("{} {e}", t("Failed:"))));
                }
            }
        });
    }

    fn load_threads(&self) {
        if let Some(peer) = self.peer() {
            self.load(move |n| async move {
                n.sms_threads(peer, 50).await.map(Msg::Threads).map_err(|e| e.to_string())
            });
        }
    }

    fn load_thread(&self, id: String) {
        if let Some(peer) = self.peer() {
            self.load(move |n| async move {
                n.sms_messages(peer, id.clone(), None, 50)
                    .await
                    .map(|m| Msg::SmsMessages(id, m))
                    .map_err(|e| e.to_string())
            });
        }
    }

    fn start_pairing(&mut self) {
        let Some(node) = self.node.clone() else { return };
        self.pair_link = None;
        let tx = self.tx.clone();
        self.pair_task = Some(self.rt.spawn(async move {
            let mut events = node.events();
            match node.pairing_start_qr().await {
                Ok(link) => {
                    let _ = tx.send(Msg::PairLink(link.to_uri()));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Line(format!("{} {e}", t("Pairing failed:"))));
                    let _ = tx.send(Msg::PairEnded);
                    return;
                }
            }
            loop {
                match events.recv().await {
                    Ok(NodeEvent::Pairing(PairingEvent::Paired(d))) => {
                        let _ = tx.send(Msg::Line(format!("✓ {} {}.", t("Paired with"), d.display_name)));
                        let _ = tx.send(Msg::AskProfile(d.id, d.display_name));
                        break;
                    }
                    Ok(NodeEvent::Pairing(PairingEvent::SasCode { code, .. })) => {
                        let _ = tx.send(Msg::Sas(code));
                    }
                    Ok(NodeEvent::Pairing(PairingEvent::Failed(why))) => {
                        let _ = tx.send(Msg::Line(format!("{} {why:?}", t("Pairing failed:"))));
                        break;
                    }
                    Ok(_) | Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                }
            }
            let _ = tx.send(Msg::PairEnded);
        }));
    }

    fn start_mirror(&mut self) {
        let Some(peer) = self.peer() else { return };
        let sound = self.mirror_sound;
        let (awake, dark) =
            (self.mirror_screen == MirrorScreen::KeepOn, self.mirror_screen == MirrorScreen::Off);
        self.mirror_wanted = true;
        self.phone_window = false;
        let (itx, mut irx) = mpsc::unbounded_channel::<MirrorInput>();
        self.mirror_input = Some(itx);
        let video = settings::Settings::load();
        let fps = video.video_fps();
        *self.platform.mirror.seen.lock().unwrap() = Some(std::time::Instant::now());
        let run = self.platform.mirror.run.fetch_add(1, Ordering::AcqRel) + 1;
        let (shared, tx) = (self.platform.mirror.clone(), self.tx.clone());
        self.act("mirror", move |n| async move {
            let options = MirrorStart {
                max_size: video.video_max_size(),
                fps,
                bitrate: video.video_bitrate(),
                audio: sound,
                session: 0,
                app: None,
                stay_awake: awake,
                screen_off: dark,
            };
            n.mirror_start(peer, options).await.map_err(|e| e.to_string())?;
            // Nobody looking for a while (minimized, or on another page): stop, so it doesn't
            // keep the phone and this PC busy unnoticed. Not with its sound on: that is listened to.
            if !sound {
                let n3 = n.clone();
                tokio::spawn(async move {
                    let mut was_active = false;
                    let mut hidden_for = std::time::Duration::ZERO;
                    loop {
                        tokio::time::sleep(WATCH).await;
                        // Stopped by hand, or asked for again: that one has its own watch.
                        if shared.run.load(Ordering::Acquire) != run {
                            break;
                        }
                        // Ended from the phone: nothing left to stop.
                        let active = shared.active.load(Ordering::Acquire);
                        if was_active && !active {
                            break;
                        }
                        // Not accepted on the phone yet: nothing to stop, and the wait doesn't count.
                        if !active {
                            continue;
                        }
                        if !was_active {
                            was_active = true;
                            *shared.seen.lock().unwrap() = Some(std::time::Instant::now());
                        }
                        let win = shared.window.load(Ordering::Acquire);
                        hidden_for = if win != 0 && island::window_hidden(win) {
                            hidden_for + WATCH
                        } else {
                            std::time::Duration::ZERO
                        };
                        let unseen = shared.seen.lock().unwrap().is_none_or(|at| at.elapsed() >= UNSEEN);
                        if hidden_for < HIDDEN && !unseen {
                            continue;
                        }
                        n3.mirror_stop(peer, 0).await;
                        let why = t(if unseen {
                            "Stopped showing the phone's screen: nobody had looked at it for 3 minutes."
                        } else {
                            "Stopped showing the phone's screen: Broche was minimized."
                        });
                        let _ = tx.send(Msg::MirrorCut(run, why));
                        break;
                    }
                });
            }
            let n2 = n.clone();
            tokio::spawn(async move {
                while let Some(input) = irx.recv().await {
                    if n2.mirror_input(peer, 0, input).await.is_err() {
                        break;
                    }
                }
            });
            Ok(t("Accept the request on the phone."))
        });
    }

    /// "Phone only": the window shrinks to the phone and shows just its screen, like a phone
    /// mirroring window. Returns whether it drew the whole window. Not a second window: egui
    /// draws those from this one's frames, and a minimized Wayland window gets none, so the
    /// phone would freeze as soon as Broche was minimized.
    fn phone_window(&mut self, ctx: &egui::Context) -> bool {
        let active = self.platform.mirror.active.load(Ordering::Acquire);
        if !self.phone_window || !active {
            if let Some(size) = self.full_size.take() {
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(420.0, 480.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("Broche".into()));
            }
            // Keep asking while the stream starts; drop the request once it has ended.
            self.phone_window &= self.mirror_wanted || active;
            return false;
        }
        self.pull_mirror_frame(ctx, active);
        if self.full_size.is_none() {
            let full =
                ctx.input(|i| i.viewport().inner_rect.map(|r| r.size())).unwrap_or(egui::vec2(1000.0, 700.0));
            self.full_size = Some(full);
            let phone = self.mirror_tex.as_ref().map_or(egui::vec2(9.0, 19.5), |t| t.size_vec2());
            let height = full.y.clamp(480.0, 860.0);
            let width = (height - 56.0) * phone.x / phone.y;
            let name =
                self.devices.iter().find(|d| Some(&d.id) == self.selected.as_ref()).map(|d| d.name.clone());
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(160.0, 240.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(width.max(200.0), height)));
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "{} · Broche",
                name.unwrap_or_else(|| t("Phone"))
            )));
        }
        egui::TopBottomPanel::bottom("phone-keys").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (label, key, tip) in
                    [("◀", "back", t("Back")), ("🏠", "home", t("Home")), ("▣", "recents", t("Recents"))]
                {
                    if kit::icon_button(ui, label, &tip).clicked()
                        && let Some(tx) = &self.mirror_input
                    {
                        let _ = tx.send(MirrorInput::Key { key: key.into() });
                    }
                }
                let dark = self.mirror_screen == MirrorScreen::Off;
                let tip =
                    if dark { t("Turn the phone's screen on") } else { t("Turn the phone's screen off") };
                if kit::icon_button(ui, if dark { "☀" } else { "🌙" }, &tip).clicked() {
                    self.set_mirror_screen(if dark { MirrorScreen::AsUsual } else { MirrorScreen::Off });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if kit::tonal_button(ui, &t("Back to Broche")).clicked() {
                        self.phone_window = false;
                    }
                });
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ctx, |ui| self.mirror_surface(ui));
        true
    }

    /// Changes what the phone's own screen does while it is being shown, right away.
    fn set_mirror_screen(&mut self, screen: MirrorScreen) {
        self.mirror_screen = screen;
        let Some(peer) = self.peer() else { return };
        let (awake, dark) = (screen == MirrorScreen::KeepOn, screen == MirrorScreen::Off);
        self.act("mirror power", move |n| async move {
            n.mirror_power(peer, awake, dark).await.map(|_| t("Done.")).map_err(|e| e.to_string())
        });
    }

    /// The options for the phone's own screen; `live` applies a change to the screen being shown.
    fn mirror_screen_choice(&mut self, ui: &mut egui::Ui, live: bool) {
        let mut screen = self.mirror_screen;
        let changed = kit::segmented(
            ui,
            &mut screen,
            &[
                (MirrorScreen::AsUsual, t("As usual")),
                (MirrorScreen::KeepOn, t("Keep it on")),
                (MirrorScreen::Off, t("Turn it off")),
            ],
        );
        if changed && live {
            self.set_mirror_screen(screen);
        } else {
            self.mirror_screen = screen;
        }
    }

    fn stop_mirror(&mut self) {
        self.platform.mirror.run.fetch_add(1, Ordering::AcqRel);
        self.mirror_wanted = false;
        self.mirror_input = None;
        if let Some(peer) = self.peer() {
            self.act("mirror stop", move |n| async move {
                n.mirror_stop(peer, 0).await;
                Ok(t("Done."))
            });
        }
    }

    /// Testing aid, `BROCHE_SHOT=<dir>`: visit the pages, save a picture of each,
    /// and quit. `BROCHE_SHOT_PAGES=screen,camera` picks pages; `BROCHE_SHOT_WAIT`
    /// is how many seconds to wait on each one (default 2).
    fn shots(&mut self, ctx: &egui::Context) {
        let Some(dir) = self.shot_dir.clone() else { return };
        const ALL: [(Tab, &str); 14] = [
            (Tab::Devices, "home"),
            (Tab::Messages, "messages"),
            (Tab::Calls, "calls"),
            (Tab::Notifications, "notifications"),
            (Tab::Screen, "screen"),
            (Tab::Camera, "camera"),
            (Tab::Files, "files"),
            (Tab::History, "history"),
            (Tab::Apps, "apps"),
            (Tab::Photos, "photos"),
            (Tab::Perms, "preferences"),
            (Tab::Access, "permissions"),
            (Tab::Pair, "pair"),
            (Tab::About, "about"),
        ];
        let wanted = std::env::var("BROCHE_SHOT_PAGES").unwrap_or_default();
        let pages: Vec<(Tab, &str)> = ALL
            .iter()
            .copied()
            .filter(|(_, n)| wanted.is_empty() || wanted.split(',').any(|w| w.trim() == *n))
            .collect();
        let wait = std::env::var("BROCHE_SHOT_WAIT").ok().and_then(|w| w.parse::<f64>().ok()).unwrap_or(2.0);
        if pages.is_empty() {
            return;
        }
        for ev in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = ev {
                let (w, h) = (image.width() as u32, image.height() as u32);
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(img) = image::RgbaImage::from_raw(w, h, rgba) {
                    let _ = std::fs::create_dir_all(&dir);
                    let name = pages[self.shot_step.min(pages.len() - 1)].1;
                    let _ = img.save(dir.join(format!("{:02}-{name}.png", self.shot_step)));
                }
                self.shot_step += 1;
                self.frame_no = 0;
                self.shot_since = std::time::Instant::now();
                if self.shot_step >= pages.len() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
        }
        let tab = pages[self.shot_step.min(pages.len() - 1)].0;
        if self.tab != tab {
            self.go_tab(tab);
            self.shot_since = std::time::Instant::now();
        }
        self.frame_no += 1;
        if self.frame_no > 1 && self.shot_since.elapsed().as_secs_f64() >= wait {
            self.frame_no = 0;
            self.shot_since = std::time::Instant::now() + Duration::from_secs(3600);
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }

    /// Testing aid, `BROCHE_AUTOSTART=screen`, `camera`, `shelf` or `app:<package>`: start that stream as soon
    /// as the phone is online (no clicking needed).
    fn autostart(&mut self, ctx: &egui::Context) {
        let Some(what) = self.autostart.clone() else { return };
        let online = self
            .devices
            .iter()
            .any(|d| Some(&d.id) == self.selected.as_ref() && d.state.starts_with("online"));
        if !online {
            return;
        }
        self.autostart = None;
        match what.as_str() {
            "screen" => self.start_mirror(),
            "camera" => {
                if let Some(peer) = self.peer() {
                    self.start_webcam(peer);
                }
            }
            "shelf" => self.toggle_shelf(ctx),
            app if app.starts_with("app:") => {
                if let Some(peer) = self.peer() {
                    let pkg = app["app:".len()..].to_string();
                    let app = broche_core::PhoneApp { label: pkg.clone(), pkg, icon: None };
                    self.open_app(ctx, peer, app);
                }
            }
            _ => {}
        }
    }

    fn start_webcam(&mut self, peer: DeviceId) {
        self.cam_wanted = true;
        // The phone keeps its own choice of camera; this is only the protocol default.
        let camera = "back";
        let fps = settings::Settings::load().video_fps();
        self.act("webcam", move |n| async move {
            let options = broche_core::WebcamStart {
                camera: camera.into(),
                width: 1280,
                height: 720,
                fps,
                bitrate: 4_000_000,
            };
            n.webcam_start(peer, options).await.map_err(|e| e.to_string())?;
            Ok(t("Open the Broche notification on the phone and tap Start webcam."))
        });
    }

    fn go_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.on_enter();
    }

    /// Loads what a page shows when it is opened.
    fn on_enter(&mut self) {
        match self.tab {
            Tab::Messages => self.load_threads(),
            Tab::Calls => {
                if let Some(peer) = self.peer() {
                    self.load(move |n| async move {
                        n.call_log(peer, None, 40).await.map(Msg::CallLog).map_err(|e| e.to_string())
                    });
                    self.load(move |n| async move {
                        n.contacts(peer, None, 0, 50).await.map(Msg::Contacts).map_err(|e| e.to_string())
                    });
                }
            }
            Tab::History => self.history.stale = true,
            Tab::Apps => self.load_apps(),
            Tab::Photos => {
                if let Some(peer) = self.peer() {
                    self.load(move |n| async move {
                        n.photo_albums(peer).await.map(Msg::Albums).map_err(|e| e.to_string())
                    });
                }
            }
            Tab::Files => {
                self.storage_loaded = false;
                self.storage_error = None;
            }
            _ => {}
        }
    }

    /// Once a second: check how each stream is doing (logged with `BROCHE_STREAM_LOG=1`), and ask the phone for a fresh
    /// keyframe when the picture was lost (decoder errors, dropped packets or no
    /// frames while packets arrive), which is what un-freezes it.
    fn maintain_streams(&mut self) {
        if self.last_check.elapsed() < Duration::from_secs(1) {
            return;
        }
        let secs = self.last_check.elapsed().as_secs_f64().max(0.1);
        self.last_check = std::time::Instant::now();
        let peer = self.peer();
        for cam in [false, true] {
            let shared = if cam { self.platform.cam.clone() } else { self.platform.mirror.clone() };
            if !shared.active.load(Ordering::Acquire) {
                continue;
            }
            let now = shared.counters();
            let before = if cam { self.snap_cam } else { self.snap_screen };
            let d = |a: u64, b: u64| a.saturating_sub(b);
            let (pk, key, dec) = (
                d(now.packets, before.packets),
                d(now.keyframes, before.keyframes),
                d(now.decoded, before.decoded),
            );
            let (err, drop) = (d(now.errors, before.errors), d(now.dropped, before.dropped));
            let text = format!(
                "{:.0} pkt/s · {} key · {:.0} fps shown · {:.1} Mbit/s · {} decoder errors · {} dropped",
                pk as f64 / secs,
                key,
                dec as f64 / secs,
                d(now.bytes, before.bytes) as f64 * 8.0 / secs / 1e6,
                err,
                drop
            );
            let text = format!(
                "{text} · {} replaced · longest gap: network {} ms, picture {} ms",
                now.replaced.saturating_sub(before.replaced),
                shared.packet_gap.take(),
                shared.picture_gap.take()
            );
            // Testing aid: `BROCHE_STREAM_LOG=1` writes these figures to the log every second.
            if std::env::var_os("BROCHE_STREAM_LOG").is_some() {
                eprintln!("{} {text}", if cam { "camera" } else { "screen" });
            }
            let stalled = pk > 0 && dec == 0;
            if (err > 0 || drop > 0 || stalled)
                && let (Some(node), Some(peer)) = (self.node.clone(), peer)
            {
                self.rt.spawn(async move {
                    if cam {
                        node.webcam_keyframe(peer).await;
                    } else {
                        node.mirror_keyframe(peer, 0).await;
                    }
                });
            }
            if cam {
                self.snap_cam = now;
            } else {
                self.snap_screen = now;
            }
        }
    }

    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::SystemDark(dark) => self.system_dark = Some(dark),
                Msg::Motion(on) => theme::set_motion(ctx, on),
                Msg::HistoryChanged => self.history.stale = true,
                Msg::ToggleShelf => self.toggle_shelf(ctx),
                Msg::DownloadProgress(id, done, total) => {
                    self.downloads.progress(id, done, total);
                    self.sync_island_download(id);
                }
                Msg::DownloadFinished(id, result) => {
                    self.downloads.finish(id, result);
                    self.sync_island_download(id);
                }
                Msg::Transfer(tr) => {
                    if self.file_transfers.accept_report(&tr) {
                        self.record_transfer(*tr);
                    }
                }
                Msg::UploadFailed(peer, id, error) => {
                    if let Some(transfer) = self.file_transfers.fail_upload(peer, &id, error) {
                        self.record_transfer(transfer);
                    }
                }
                Msg::PhoneApps(peer, apps) => self.got_apps(ctx, peer, apps),
                Msg::Doctor(found) => {
                    self.doctor = Some(found);
                    self.doctor_running = false;
                }
                Msg::AutostartFailed(on, e) => {
                    self.autostart_on = !on;
                    self.log.push(format!("{} {e}", t("Start with session:")));
                }
                Msg::VideoQuality(q) => {
                    self.experimental60 = q.high;
                    settings::Settings::update(|s| (s.experimental60, s.video_at) = (q.high, q.at));
                }
                Msg::PhoneBattery(id, b) => {
                    self.batteries.insert(id, b);
                }
                Msg::PhoneMedia(id, players) => {
                    for p in &players {
                        if let (Some(key), Some(bytes)) = (&p.art_key, &p.art)
                            && let Ok(img) = image::load_from_memory(bytes)
                        {
                            let rgba = img.thumbnail(128, 128).to_rgba8();
                            let ci = egui::ColorImage::from_rgba_unmultiplied(
                                [rgba.width() as usize, rgba.height() as usize],
                                rgba.as_raw(),
                            );
                            self.art.insert(
                                key.clone(),
                                ctx.load_texture(format!("art-{key}"), ci, Default::default()),
                            );
                        }
                    }
                    self.players.insert(id, players);
                    // Old pictures are only dropped once there are many: the phone won't send one twice.
                    if self.art.len() > 32 {
                        let used: Vec<String> =
                            self.players.values().flatten().filter_map(|p| p.art_key.clone()).collect();
                        self.art.retain(|k, _| used.contains(k));
                    }
                }
                Msg::Picked(_, paths) if paths.is_empty() => {}
                Msg::Picked(Pick::Send(peer), paths) => self.send_paths(peer, paths),
                Msg::Picked(Pick::Upload(peer, dir), paths) => self.upload(peer, dir, paths),
                Msg::Picked(Pick::SharedFolder, paths) => {
                    if let Some(path) = paths.into_iter().next() {
                        self.shared_edit = path.display().to_string();
                        self.log.push(format!("{} {}", t("Shared folder:"), self.shared_edit));
                        self.platform.set_shared_dir(path);
                        settings::Settings::update(|s| s.shared_dir = self.shared_edit.clone());
                    }
                }
                Msg::StorageError(e) => {
                    self.storage_loading = false;
                    self.storage_error = Some(e); // English: `files_tab` reads it, then translates it to show
                }
                Msg::StorageRefresh(dir) => {
                    if dir == self.storage_path {
                        self.list_storage(dir);
                    }
                }
                Msg::TrayAction(id) => {
                    let peer = self.peer();
                    match id {
                        5 => {
                            if let Some(peer) = peer {
                                self.toggle_ring(peer);
                            }
                        }
                        7 => {
                            if self.platform.mirror.active.load(Ordering::Acquire) || self.mirror_wanted {
                                // The window was only the phone: put it away with it, instead
                                // of turning back into the whole of Broche.
                                if self.phone_window {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                                }
                                self.stop_mirror();
                            } else {
                                // From the tray the phone always opens in window mode; the
                                // preference is for the button in the window.
                                self.start_mirror();
                                self.phone_window = true;
                            }
                        }
                        13 => {
                            if let Some(p) = peer {
                                self.pick(Pick::Send(p));
                            }
                        }
                        14 => {
                            if let (Some(p), Some(node)) = (peer, self.node.clone()) {
                                let on = node.phone_toggles(p).is_some_and(|tg| tg.dnd);
                                self.set_toggle(p, "dnd", PhoneToggleValue::Bool(!on));
                            }
                        }
                        15 => self.set_autostart(!settings::autostart_enabled()),
                        9 => {
                            self.platform.stop_video();
                            self.stop_mirror();
                            if let Some(p) = peer {
                                self.act("webcam stop", move |n| async move {
                                    n.webcam_stop(p).await;
                                    Ok(t("Done."))
                                });
                            }
                        }
                        10 => {
                            self.go_tab(Tab::Files);
                            // Wayland does not let an app bring its own window to the front
                            // (XWayland does: the tray already raised it).
                            if !self.on_x11 {
                                self.rt.spawn(platform::desktop_notify(
                                t("The phone's files are open in Broche. Open its window from the dock or with Alt+Tab."),
                                1,
                                ));
                            }
                        }
                        11 => self.go_tab(Tab::Perms),
                        18 => self.go_tab(Tab::About),
                        12 => self.go_tab(Tab::Pair),
                        16 => {
                            if let Some(p) = peer {
                                use broche_core::Platform;
                                let _ = self.platform.set_pc_audio(
                                    &p,
                                    Some(if self.speaker { 200 } else { 201 }),
                                    None,
                                );
                            }
                        }
                        17 => {
                            let _ = std::process::Command::new("xdg-open").arg(&self.shared_edit).spawn();
                        }
                        _ => {}
                    }
                }
                Msg::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Msg::OpenReceivedFolder => tray::open_folder(&received_dir()),
                Msg::ShowWindow => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Msg::OpenNotifications => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    self.go_tab(Tab::Notifications);
                }
                Msg::Refresh => self.refresh(),
                // Unless it was asked for again meanwhile: forget it, like stopping by hand.
                Msg::MirrorCut(run, why) => {
                    if self.platform.mirror.run.load(Ordering::Acquire) == run {
                        self.mirror_wanted = false;
                        self.mirror_input = None;
                    }
                    self.log.push(why);
                }
                Msg::Line(l) => {
                    self.log.push(i18n::core_errors(&l));
                    if self.log.len() > 2000 {
                        self.log.drain(..500);
                    }
                }
                Msg::AskProfile(id, name) => self.profile_prompt = Some((id, name, PermissionProfile::Basic)),
                Msg::PairLink(l) => self.pair_link = Some(l),
                Msg::Sas(code) => {
                    if self.pair_task.is_none() {
                        // Not from the QR on screen: bring the window up where the code is shown.
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.go_tab(Tab::Pair);
                    }
                    self.sas = Some(code);
                }
                Msg::PairEnded => {
                    self.pair_task = None;
                    self.pair_link = None;
                    self.sas = None;
                    self.refresh();
                }
                Msg::Clipboard(text) => {
                    // The phone echoing back what this PC just sent is not news.
                    let echo = self.last_clip.as_ref() == Some(&text);
                    self.last_clip = Some(text.clone());
                    ctx.copy_text(text);
                    self.log.push(t("Clipboard received."));
                    if self.clipboard_auto && !echo {
                        // Only that something arrived: what was copied stays private.
                        self.rt.spawn(platform::desktop_notify(t("New clipboard from the phone."), 1));
                    }
                }
                Msg::ClipboardImage(image) => {
                    ctx.copy_image(image);
                    self.log.push(t("Clipboard received."));
                }
                Msg::LocalCopy(text) => {
                    let online = self
                        .devices
                        .iter()
                        .any(|d| Some(&d.id) == self.selected.as_ref() && d.state.starts_with("online"));
                    if self.clipboard_auto
                        && online
                        && self.last_clip.as_ref() != Some(&text)
                        && let Some(peer) = self.peer()
                    {
                        self.last_clip = Some(text.clone());
                        self.act("clipboard", move |n| async move {
                            n.send_clipboard(peer, text)
                                .await
                                .map(|_| t("Clipboard sent."))
                                .map_err(|e| e.to_string())
                        });
                    }
                }
                Msg::SpeakerChanged(on) => self.speaker = on,
                Msg::AudioChanged => self.pc_volume = platform::pc_volume(),
                Msg::Threads(v) => self.threads = v,
                Msg::SmsMessages(id, v) => {
                    if self.open_thread.as_deref() == Some(id.as_str()) {
                        self.sms_messages = v;
                    }
                }
                Msg::SmsDirty => {
                    if self.tab == Tab::Messages {
                        self.load_threads();
                        if let Some(id) = self.open_thread.clone() {
                            self.load_thread(id);
                        }
                    }
                }
                Msg::CallLog(v) => self.calls = v,
                Msg::Contacts(v) => self.contacts = v,
                Msg::Albums(v) => self.albums = v,
                Msg::PhotoItems(v) => {
                    let ids: Vec<String> = v.iter().take(60).map(|p| p.id.clone()).collect();
                    self.photos = v;
                    if let Some(peer) = self.peer() {
                        self.load(move |n| async move {
                            n.photo_thumbs(peer, ids).await.map(Msg::Thumbs).map_err(|e| e.to_string())
                        });
                    }
                }
                Msg::Thumbs(thumbs) => {
                    for th in thumbs {
                        if let Ok(img) = image::load_from_memory(&th.data) {
                            let rgba = img.to_rgba8();
                            let ci = egui::ColorImage::from_rgba_unmultiplied(
                                [rgba.width() as usize, rgba.height() as usize],
                                rgba.as_raw(),
                            );
                            let tex = ctx.load_texture(format!("thumb-{}", th.id), ci, Default::default());
                            self.thumbs.insert(th.id, tex);
                        }
                    }
                }
                Msg::Notif(n) => {
                    self.notifications.retain(|x| x.key != n.key);
                    self.notifications.insert(0, *n);
                }
                Msg::NotifReset(items) => self.notifications = items,
                Msg::NotifGone(key) => self.notifications.retain(|x| x.key != key),
                Msg::Ringing(on) => self.ringing = on,
                Msg::RingFrom(peer) => self.ring_from = peer,
                Msg::Captured(path) => {
                    self.log.push(format!("{} {}", t("Saved:"), path.display()));
                    self.last_capture = Some(path);
                }
                Msg::PcMirrorAsk(d) => self.mirror_ask = Some(d),
                Msg::Received(paths) => {
                    self.log.push(format!("{} {paths}", t("Received:")));
                    self.received.insert(0, paths);
                    self.received.truncate(30);
                }
                Msg::StorageList(path, mut entries) => {
                    entries.sort_by(|a, b| {
                        b.is_dir
                            .cmp(&a.is_dir)
                            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                    });
                    if path != self.storage_path {
                        self.storage_selected.clear();
                    }
                    self.storage_path = path;
                    self.storage_entries = entries;
                    self.storage_loaded = true;
                    self.storage_loading = false;
                    self.storage_error = None;
                }
                Msg::Call(c) => {
                    let over = matches!(c.state.as_str(), "idle" | "ended" | "disconnected" | "none");
                    self.call = (!over).then_some(c);
                }
            }
        }
    }

    // ---- Tabs ----

    fn toggles_ui(&mut self, ui: &mut egui::Ui, peer: DeviceId, tg: PhoneToggles) {
        kit::title(ui, &t("Phone controls"));
        // The volume slider: icon, bar and value on one line.
        ui.horizontal(|ui| {
            kit::glyph(ui, "🔊", 18.0, ui.visuals().text_color()).on_hover_text(t("Volume"));
            ui.spacing_mut().slider_width = (ui.available_width() - 80.0).max(120.0);
            let mut v = f32::from(tg.volume);
            let r = ui.add(egui::Slider::new(&mut v, 0.0..=100.0).integer().show_value(false));
            let r = kit::named(r, &t("Volume")).on_hover_text(t("Volume"));
            // Our own value box (the slider's built-in one can't be given a name).
            let d = ui.add(egui::DragValue::new(&mut v).range(0.0..=100.0).max_decimals(0).suffix(" %"));
            let d = kit::named(d, &t("Volume"));
            let done = |r: &egui::Response| r.drag_stopped() || (r.changed() && !r.dragged());
            if done(&r) || done(&d) {
                self.set_toggle(peer, "volume", PhoneToggleValue::Level(v as u8));
            }
        });
        ui.add_space(4.0);
        // Sound: one choice. "Silent" also turns on Do not disturb (the same idea for people).
        let current: &'static str = if tg.dnd || tg.ringer == "silent" {
            "silent"
        } else if tg.ringer == "vibrate" {
            "vibrate"
        } else {
            "ring"
        };
        let mut mode = current;
        if kit::segmented(
            ui,
            &mut mode,
            &[
                ("ring", format!("🔔 {}", t("Ring"))),
                ("vibrate", format!("📳 {}", t("Vibrate"))),
                ("silent", format!("🔕 {}", t("Silent"))),
            ],
        ) {
            self.set_toggle(peer, "ringer", PhoneToggleValue::Mode(mode.into()));
            let dnd = mode == "silent";
            if dnd != tg.dnd {
                self.set_toggle(peer, "dnd", PhoneToggleValue::Bool(dnd));
            }
        }
        kit::supporting(ui, t("Silent also turns on Do not disturb."));
        // What the phone can do changes rarely: look it up every few seconds.
        if self
            .caps_cache
            .as_ref()
            .is_none_or(|(p, at, _)| *p != peer || at.elapsed() > Duration::from_secs(3))
        {
            let matrix = self.node.as_ref().and_then(|n| n.capabilities(peer).ok());
            let can = |feature: &str| {
                matrix
                    .as_ref()
                    .and_then(|m| m.state(feature))
                    .is_some_and(|s| matches!(s, broche_core::FeatureState::Available))
            };
            self.caps_cache =
                Some((peer, std::time::Instant::now(), (can("toggles.wifi"), can("toggles.bluetooth"))));
        }
        let (can_wifi, can_bt) = self.caps_cache.as_ref().map_or((false, false), |(_, _, c)| *c);
        if can_wifi {
            let mut wifi = tg.wifi;
            if kit::switch_row(ui, &mut wifi, &t("Wi-Fi"), None) {
                self.set_toggle(peer, "wifi", PhoneToggleValue::Bool(wifi));
            }
        }
        if can_bt {
            let mut bt = tg.bluetooth;
            if kit::switch_row(ui, &mut bt, &t("Bluetooth"), None) {
                self.set_toggle(peer, "bluetooth", PhoneToggleValue::Bool(bt));
            }
        }
    }

    /// Whether we asked the phone to ring (it stops by itself after a while, or when found).
    pub(crate) fn phone_ringing(&self) -> bool {
        self.tray_state
            .read()
            .ok()
            .and_then(|s| s.ring_started)
            .is_some_and(|t| t.elapsed() < Duration::from_secs(30))
    }

    /// Sends the phone's sound to this PC over Bluetooth (`on`), or back to the phone, and plays
    /// its music again if it was playing: Android pauses it when the output changes. Asks for a
    /// state, not a flip, so a repeated request does no harm.
    pub(crate) fn set_bt_music(&mut self, on: bool) {
        // Without the node act() does nothing, and the claim would never be released.
        if self.node.is_none() || bt_music::current().is_none_or(|p| p.on == on) || !bt_music::claim() {
            return;
        }
        // Both at once would loop the sound between the PC and the phone.
        if on
            && self.speaker
            && let Some(peer) = self.peer()
        {
            use broche_core::Platform;
            let _ = self.platform.set_pc_audio(&peer, Some(broche_core::deck::PHONE_SPEAKER_OFF), None);
        }
        let resume = self.peer().and_then(|peer| {
            let player = self.players.get(&peer.to_string())?.iter().find(|p| p.playing)?;
            Some((peer, player.id.clone()))
        });
        self.act("bluetooth sound", move |n| {
            bt_music::switch(on, resume.map(|(peer, player)| (n, peer, player)))
        });
    }

    /// Find my phone: rings, or stops the ringing if it's already on.
    pub(crate) fn toggle_ring(&mut self, peer: DeviceId) {
        let on = !self.phone_ringing();
        if let Ok(mut st) = self.tray_state.write() {
            st.ring_started = on.then(std::time::Instant::now);
        }
        self.act("ring", move |n| async move {
            n.ring(peer, on)
                .await
                .map(|_| t(if on { "Ringing…" } else { "Stopped ringing." }))
                .map_err(|e| e.to_string())
        });
    }

    /// What this phone may do on the PC: one card per group, each permission explained.
    /// The phone's permissions, from the database at most every 2 s (not on every frame).
    fn cached_toggles(&mut self, peer: DeviceId) -> Option<Vec<(&'static str, bool)>> {
        let node = self.node.clone()?;
        let fresh = self
            .perm_cache
            .as_ref()
            .is_some_and(|(p, at, _)| *p == peer && at.elapsed() < Duration::from_secs(2));
        if !fresh {
            let list = node.device_toggles(peer).ok()?;
            self.perm_cache = Some((peer, std::time::Instant::now(), list));
        }
        self.perm_cache.as_ref().map(|(_, _, l)| l.clone())
    }

    /// The permission profile picker for `peer`, applied as soon as one is chosen.
    fn profile_card(&mut self, ui: &mut egui::Ui, peer: DeviceId, current: Option<PermissionProfile>) {
        let Some(node) = self.node.clone() else { return };
        kit::card(ui, |ui| {
            kit::title(ui, &t("Profile"));
            kit::supporting(
                ui,
                t("Choose what this phone may do with this PC. You can change it at any time."),
            );
            if current.is_none() {
                kit::supporting(ui, t("Custom: some permissions were changed by hand."));
            }
            if let Some(profile) = profile_choices(ui, current)
                && Some(profile) != current
            {
                self.perm_cache = None;
                if let Err(e) = node.apply_permission_profile(peer, profile) {
                    self.log.push(e.to_string());
                }
            }
        });
    }

    fn permissions_ui(&mut self, ui: &mut egui::Ui, peer: DeviceId) {
        let Some(node) = self.node.clone() else { return };
        let Some(list) = self.cached_toggles(peer) else { return };
        let current = broche_core::features::profile_of(&list);
        self.profile_card(ui, peer, current);
        egui::CollapsingHeader::new(t("Detailed permissions"))
            .id_salt("detailed-permissions")
            .default_open(current.is_none())
            .show(ui, |ui| {
                for (label, about, keys) in PERM_GROUPS {
                    kit::card(ui, |ui| {
                        kit::title(ui, &t(label));
                        kit::supporting(ui, t(about));
                        ui.add_space(4.0);
                        let mut first = true;
                        for &(name, on) in &list {
                            if !keys.contains(&name) {
                                continue;
                            }
                            if !first {
                                kit::divider(ui);
                            }
                            first = false;
                            let mut want = on;
                            let help = i18n::perm_help(name);
                            if kit::switch_row(
                                ui,
                                &mut want,
                                &t(&format!("perm:{name}")),
                                (!help.is_empty()).then_some(help.as_str()),
                            ) {
                                self.perm_cache = None;
                                if let Err(e) = node.set_device_toggle(peer, name, want) {
                                    self.log.push(e.to_string());
                                }
                            }
                        }
                    });
                }
                kit::supporting(ui, t("The first time, your desktop asks you to approve remote control."));
            });
    }

    /// Notices that need attention whatever the tab: ringing, a screen request.
    fn banners(&mut self, ctx: &egui::Context) {
        if !self.ringing && self.mirror_ask.is_none() {
            return;
        }
        let fill = {
            let p = theme::palette_for(self.scheme, ctx.style().visuals.dark_mode);
            p.error_container
        };
        egui::TopBottomPanel::top("banners")
            .frame(egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(16, 10)))
            .show(ctx, |ui| {
                let fg = theme::pal(ui).on_error_container;
                if self.ringing {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(t("Your phone is looking for this PC.")).strong().color(fg),
                        );
                        if kit::filled_button(ui, &t("Stop ringing")).clicked() {
                            self.platform.stop_ringing_now();
                            // Tell the phone, so it stops offering "Stop ringing".
                            if let (Some(n), Some(peer)) = (self.node.clone(), self.ring_from.take()) {
                                self.rt.spawn(async move { n.notify_ring_stopped(peer).await });
                            }
                        }
                    });
                }
                if let Some(peer) = self.mirror_ask {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(t("Your phone wants to see this PC's screen."))
                                .strong()
                                .color(fg),
                        );
                        if kit::filled_button(ui, &t("Allow once")).clicked() {
                            if let Some(n) = &self.node {
                                n.set_pc_mirror_session_allowed(peer, true);
                            }
                            self.mirror_ask = None;
                        }
                        if kit::tonal_button(ui, &t("Always allow")).clicked() {
                            if let Some(n) = &self.node {
                                let _ = n.set_device_toggle(peer, "pc_mirror", true);
                            }
                            self.mirror_ask = None;
                        }
                        if kit::danger_button(ui, &t("Deny")).clicked() {
                            self.mirror_ask = None;
                        }
                    });
                }
            });
    }

    /// What a page that needs a phone shows without one: how to pair the first, or where
    /// to choose one.
    pub(crate) fn no_phone(&mut self, ui: &mut egui::Ui) {
        if !self.devices.is_empty() {
            kit::card(ui, |ui| {
                kit::empty_state(ui, "📱", &t("No phone chosen."), &t("Choose a phone at the top right."))
            });
            return;
        }
        kit::card(ui, |ui| {
            kit::empty_state(
                ui,
                "📱",
                &t("Link your phone"),
                &t("Open Broche on your phone and scan the code that appears here."),
            );
            ui.vertical_centered(|ui| {
                if kit::filled_button(ui, &t("Show pairing code")).clicked() {
                    self.go(Tab::Pair);
                    self.start_pairing();
                }
            });
        });
    }

    fn camera_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(peer) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        let shared = self.platform.cam.clone();
        let active = shared.active.load(Ordering::Acquire);
        if self.cam_was_active && !active {
            self.cam_wanted = false;
            self.cam_tex = None;
        }
        self.cam_was_active = active;
        if let Some(img) = shared.frame.lock().unwrap().take() {
            if std::mem::take(&mut self.cam_snap) {
                save_frame(&img, self.tx.clone());
            }
            match &mut self.cam_tex {
                Some(tex) => tex.set(img, Default::default()),
                None => self.cam_tex = Some(ctx.load_texture("cam", img, Default::default())),
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            kit::card(ui, |ui| {
                kit::title(ui, &t("Phone camera"));
                kit::supporting(ui, t("Choose the front or back camera, zoom and flash on the phone, in Broche › Webcam. Photos and scans are saved in Broche/Recibidos."));
                kit::supporting(ui, t("The camera shows only here, in Broche: video-call apps can't use it as a camera."));
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if !active && !self.cam_wanted {
                        if kit::filled_button(ui, &format!("📷 {}", t("Start webcam"))).clicked() {
                            self.start_webcam(peer);
                        }
                    } else if kit::danger_button(ui, &format!("■ {}", t("Stop webcam"))).clicked() {
                        self.cam_wanted = false;
                        self.act("webcam stop", move |n| async move {
                            n.webcam_stop(peer).await;
                            Ok(t("Done."))
                        });
                    }
                    for (label, mode) in [
                        (format!("📸 {}", t("Take photo")), broche_core::CameraCaptureMode::Photo),
                        (format!("📄 {}", t("Scan document")), broche_core::CameraCaptureMode::Scan),
                    ] {
                        if kit::tonal_button(ui, &label).clicked() {
                            // While the webcam runs, asking the phone for a still froze the stream
                            // (and its torch): a photo is the frame already showing, and a scan
                            // stops the webcam first.
                            if active && mode == broche_core::CameraCaptureMode::Photo {
                                self.cam_snap = true;
                                continue;
                            }
                            if active {
                                self.cam_wanted = false;
                            }
                            let id = format!(
                                "pc-{}",
                                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis())
                            );
                            self.act("camera capture", move |n| async move {
                                if active {
                                    n.webcam_stop(peer).await;
                                }
                                n.request_camera_capture(peer, id, mode).await.map_err(|e| e.to_string())?;
                                Ok(t("Open the Broche notification on the phone to take it."))
                            });
                        }
                    }
                });
                if self.cam_wanted && !active {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        kit::supporting(ui, t("Open the Broche notification on the phone and tap Start webcam."));
                    });
                }
                if let Some(path) = self.last_capture.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(format!("{} {}", t("Saved:"), path.display())).small());
                        if kit::tonal_button(ui, &t("Open")).clicked() {
                            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                        }
                    });
                }
                if active && let Some(tex) = &self.cam_tex {
                    let size = tex.size_vec2();
                    let max = egui::vec2(ui.available_width(), (ui.ctx().content_rect().height() - 300.0).max(200.0));
                    let scale = (max.x / size.x).min(max.y / size.y).clamp(0.05, 2.0);
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(size * scale).corner_radius(12));
                    });
                }
            });
        });
    }

    fn list_storage(&mut self, path: String) {
        // Testing aid: `BROCHE_TEST_STORAGE=1` shows made-up folders and files.
        if std::env::var_os("BROCHE_TEST_STORAGE").is_some() {
            let e = |name: &str, size: u64, is_dir: bool| StorageEntry {
                name: name.into(),
                size,
                modified: 0,
                is_dir,
            };
            let entries = vec![
                e("Download", 0, true),
                e("DCIM", 0, true),
                e("Documents", 0, true),
                e("Music", 0, true),
                e("Pictures", 0, true),
                e(".thumbnails", 0, true),
                e("Informe trimestral de ventas 2026 versión final.pdf", 2_345_678, false),
                e("foto_playa.jpg", 3_456_789, false),
                e("cancion.mp3", 5_123_456, false),
                e("notas.txt", 1_234, false),
                e("app-release.apk", 45_678_901, false),
                e("copia.zip", 123_456_789, false),
                e("video.mp4", 98_765_432, false),
            ];
            let _ = self.tx.send(Msg::StorageList(path, entries));
            return;
        }
        let Some(peer) = self.peer() else { return };
        let Some(node) = self.node.clone() else { return };
        self.storage_loading = true;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let msg = match node.storage_list(peer, path.clone()).await {
                Ok(e) => Msg::StorageList(path, e),
                Err(e) => Msg::StorageError(e.to_string()),
            };
            let _ = tx.send(msg);
        });
    }

    fn storage_join(&self, name: &str) -> String {
        if self.storage_path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", self.storage_path, name)
        }
    }

    /// Copies a file of the phone into this PC's received folder (`Phone/`), streaming it
    /// to disk (big videos don't fill the memory) under a safe, non-clashing name.
    fn record_transfer(&mut self, transfer: broche_core::Transfer) {
        let duplicate_end = transfer.state.is_finished()
            && self.file_transfers.items.iter().any(|old| {
                old.id == transfer.id
                    && old.device == transfer.device
                    && old.direction == transfer.direction
                    && old.state.is_finished()
            });
        if !duplicate_end || self.island_transfers.contains(&transfer) {
            self.island_transfers.update(&transfer);
        }
        self.file_transfers.update(&transfer);
    }

    fn sync_island_download(&mut self, id: usize) {
        if let Some(download) = self.downloads.items.get(id) {
            self.island_transfers.download(self.downloads.generation, id, download);
        }
    }

    fn download_from_phone(&mut self, peer: DeviceId, full: String, name: String, size: u64) {
        if self.node.is_none() {
            self.log.push(t("Not connected to the Broche core."));
            return;
        }
        let id = self.downloads.add(name.clone(), size, peer);
        self.sync_island_download(id);
        let tx = self.tx.clone();
        let dir = received_dir().join("Phone");
        self.act("download", move |n| async move {
            // One at a time: the phone takes only so many streams at once, and two names that
            // end up the same would otherwise share a `.part`.
            static QUEUE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
            let _turn = QUEUE.lock().await;
            let result = async {
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let dest = unique_path(&dir, &safe_file_name(&name));
                let part = dest.with_extension(format!(
                    "{}part",
                    dest.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()
                ));
                let (meta, mut recv) =
                    n.storage_read_stream(peer, full, 0, None).await.map_err(|e| e.to_string())?;
                let _ = tx.send(Msg::DownloadProgress(id, 0, meta.length));
                let mut reported = std::time::Instant::now();
                let mut got: u64 = 0;
                let result = async {
                    use std::io::Write;
                    let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
                    while let Some(chunk) = recv.read_chunk(256 * 1024).await.map_err(|e| e.to_string())? {
                        let next = got.checked_add(chunk.len() as u64).filter(|next| *next <= meta.length);
                        let Some(next) = next else {
                            return Err("the phone sent more than announced".to_string());
                        };
                        out.write_all(&chunk).map_err(|e| e.to_string())?;
                        got = next;
                        if reported.elapsed() >= std::time::Duration::from_millis(100) {
                            let _ = tx.send(Msg::DownloadProgress(id, got, meta.length));
                            reported = std::time::Instant::now();
                        }
                    }
                    if got != meta.length {
                        return Err("the transfer stopped before the end".to_string());
                    }
                    out.sync_all().map_err(|e| e.to_string())
                }
                .await;
                let _ = tx.send(Msg::DownloadProgress(id, got, meta.length));
                if let Err(e) = result {
                    let _ = std::fs::remove_file(&part);
                    return Err(e);
                }
                std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
                Ok(format!("{} {}", t("Saved:"), dest.display()))
            }
            .await;
            let _ = tx.send(Msg::DownloadFinished(id, result.as_ref().map(|_| ()).map_err(Clone::clone)));
            result
        });
    }

    /// Copies files of this PC into a folder of the phone, then shows that folder again.
    fn upload(&mut self, peer: DeviceId, dir: String, paths: Vec<PathBuf>) {
        let Some(node) = self.node.clone() else { return };
        let tx = self.tx.clone();
        self.log.push(format!("$ upload {} item(s)", paths.len()));
        self.rt.spawn(async move {
            let mut ok = 0;
            static NEXT_BATCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let sequence = NEXT_BATCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let batch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let jobs: Vec<_> = paths
                .into_iter()
                .enumerate()
                .map(|(index, src)| {
                    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let dest = if dir.is_empty() { name.clone() } else { format!("{dir}/{name}") };
                    let id = format!("gui-{}-{batch:x}-{sequence:x}-{index:x}", std::process::id());
                    let transfer = broche_core::Transfer {
                        id: id.clone(),
                        device: peer,
                        direction: broche_core::Direction::Outgoing,
                        names: vec![name.clone()],
                        files: 1,
                        total: std::fs::metadata(&src).map(|meta| meta.len()).unwrap_or(0),
                        done: 0,
                        state: TransferState::Waiting,
                        open_on_arrival: false,
                    };
                    let _ = tx.send(Msg::Transfer(Box::new(transfer)));
                    (src, dest, name, id)
                })
                .collect();
            for (src, dest, name, id) in jobs {
                match node.storage_write_with_id(peer, dest, &src, Some(id.clone()), None).await {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        let _ = tx.send(Msg::UploadFailed(peer, id, e.to_string()));
                        let _ = tx.send(Msg::Line(format!("{} {name}: {e}", t("Failed:"))));
                    }
                }
            }
            let _ = tx.send(Msg::Line(format!("{} {ok}", t("Uploaded:"))));
            let _ = tx.send(Msg::StorageRefresh(dir));
        });
    }

    fn transfer_progress(&mut self, ui: &mut egui::Ui) {
        if self.file_transfers.items.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.strong(t("File transfers"));
            let failed = self
                .file_transfers
                .items
                .iter()
                .filter(|tr| matches!(tr.state, TransferState::Failed(_)))
                .count();
            if failed > 0 {
                let what = t(if failed == 1 { "failed transfer" } else { "failed transfers" });
                ui.colored_label(theme::pal(ui).error, format!("{failed} {what}"));
            }
            if self.file_transfers.items.iter().any(|tr| tr.state.is_finished())
                && kit::icon_button(ui, "✖", &t("Dismiss")).clicked()
            {
                self.file_transfers.items.retain(|tr| !tr.state.is_finished());
            }
        });
        let p = theme::pal(ui);
        for finished in [false, true] {
            for tr in self.file_transfers.items.iter().filter(|tr| tr.state.is_finished() == finished) {
                kit::card(ui, |ui| {
                    let direction = t(if tr.direction == broche_core::Direction::Outgoing {
                        "Sending"
                    } else {
                        "Receiving"
                    });
                    let name = tr.names.first().cloned().unwrap_or_default();
                    ui.add(
                        egui::Label::new(egui::RichText::new(format!("{direction}: {name}")).strong())
                            .truncate(),
                    );
                    let state = match &tr.state {
                        TransferState::Requested => t("Waiting for approval"),
                        TransferState::Waiting => t("Waiting for the other device"),
                        TransferState::Running => direction,
                        TransferState::Paused => t("Paused"),
                        TransferState::Done { .. } => t("Transfer complete"),
                        TransferState::Cancelled => t("Cancelled"),
                        TransferState::Failed(reason) => {
                            let reason = match reason {
                                broche_core::TransferFailure::Denied => t("Transfer refused"),
                                broche_core::TransferFailure::Unreachable => t("Device unreachable"),
                                broche_core::TransferFailure::NoSpace => t("Not enough storage space"),
                                broche_core::TransferFailure::Interrupted => t("Transfer interrupted"),
                                broche_core::TransferFailure::Integrity => t("Integrity check failed"),
                                broche_core::TransferFailure::Other(error) => i18n::core_errors(error),
                            };
                            format!("{} {reason}", t("Failed:"))
                        }
                    };
                    let fraction = if matches!(tr.state, TransferState::Done { .. }) {
                        1.0
                    } else if tr.total > 0 {
                        (tr.done as f64 / tr.total as f64).clamp(0.0, 1.0) as f32
                    } else {
                        0.0
                    };
                    ui.add(egui::ProgressBar::new(fraction).show_percentage());
                    let mut amount = format!("{} / {}", human_size(tr.done), human_size(tr.total));
                    if tr.files > 1 {
                        amount.push_str(&format!(" · {} {}", tr.files, t("files")));
                    }
                    ui.label(amount);
                    if matches!(tr.state, TransferState::Failed(_)) {
                        ui.colored_label(p.error, state);
                    } else {
                        ui.label(state);
                    }
                });
                ui.add_space(8.0);
            }
        }
    }

    fn file_progress(&mut self, ui: &mut egui::Ui) {
        if self.downloads.items.is_empty() && self.file_transfers.items.is_empty() {
            return;
        }
        let p = theme::pal(ui);
        egui::ScrollArea::vertical()
            .id_salt("file-progress")
            .max_height(220.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let downloads_finished = self.downloads.finished();
                if downloads_finished {
                    self.transfer_progress(ui);
                }
                if !self.downloads.items.is_empty() {
                    let downloads = &self.downloads;
                    let (done, total) = downloads.bytes();
                    let finished = downloads.finished();
                    let mut dismiss = false;
                    kit::card(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            let title = if finished {
                                if downloads.failed() > 0 {
                                    t("Downloads finished with errors")
                                } else {
                                    t("Downloads complete")
                                }
                            } else {
                                t("Downloading files")
                            };
                            ui.label(egui::RichText::new(title).strong());
                            ui.label(format!(
                                "{} / {} {}",
                                downloads.completed(),
                                downloads.items.len(),
                                t("files completed")
                            ));
                            if finished && kit::icon_button(ui, "✖", &t("Dismiss")).clicked() {
                                dismiss = true;
                            }
                        });
                        ui.add(egui::ProgressBar::new(downloads.fraction()).show_percentage());
                        ui.label(format!("{} / {}", human_size(done), human_size(total)));
                        if let Some(current) =
                            downloads.items.iter().find(|item| item.started && item.result.is_none())
                        {
                            ui.add(egui::Label::new(&current.name).truncate());
                        }
                        if downloads.failed() > 0 {
                            let what =
                                t(if downloads.failed() == 1 { "failed file" } else { "failed files" });
                            ui.collapsing(format!("{} {what}", downloads.failed()), |ui| {
                                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                                    for item in &downloads.items {
                                        if let Some(Err(error)) = &item.result {
                                            ui.colored_label(
                                                p.error,
                                                format!("{}: {}", item.name, i18n::core_errors(error)),
                                            );
                                        }
                                    }
                                });
                            });
                        }
                    });
                    if dismiss {
                        self.downloads.items.clear();
                    }
                    ui.add_space(8.0);
                }

                if !downloads_finished {
                    self.transfer_progress(ui);
                }
            });
        ui.add_space(8.0);
    }

    fn files_tab(&mut self, ui: &mut egui::Ui) {
        let Some(peer) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        let online = self
            .devices
            .iter()
            .any(|d| Some(&d.id) == self.selected.as_ref() && d.state.starts_with("online"));
        if !self.storage_loaded
            && !self.storage_loading
            && (online || std::env::var_os("BROCHE_TEST_STORAGE").is_some())
        {
            self.storage_loaded = true;
            self.list_storage(self.storage_path.clone());
        }
        let p = theme::pal(ui);

        // Toolbar: where we are, and what can be done here.
        kit::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let root = ui.add(
                    egui::Button::new(egui::RichText::new(format!("📱 {}", t("Phone"))).strong())
                        .frame(false),
                );
                if kit::named(root, &t("Phone")).clicked() {
                    self.list_storage(String::new());
                }
                let parts: Vec<String> =
                    self.storage_path.split('/').filter(|s| !s.is_empty()).map(str::to_string).collect();
                for (i, part) in parts.iter().enumerate() {
                    kit::glyph(ui, "›", 15.0, p.on_surface_variant);
                    let last = i + 1 == parts.len();
                    let text = egui::RichText::new(part).color(if last { p.on_surface } else { p.primary });
                    if ui
                        .add(egui::Button::new(if last { text.strong() } else { text }).frame(false))
                        .clicked()
                        && !last
                    {
                        self.list_storage(parts[..=i].join("/"));
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if kit::icon_button(ui, "⬆", &t("Up")).clicked() && !self.storage_path.is_empty() {
                    let up =
                        self.storage_path.rsplit_once('/').map(|(a, _)| a.to_string()).unwrap_or_default();
                    self.list_storage(up);
                }
                if kit::icon_button(ui, "⟳", &t("Refresh")).clicked() {
                    self.list_storage(self.storage_path.clone());
                }
                let mut list = self.storage_list;
                if kit::segmented(ui, &mut list, &[(false, t("Grid")), (true, format!("☰ {}", t("List")))])
                {
                    self.storage_list = list;
                    settings::Settings::update(|s| s.storage_list = list);
                }
                if kit::chip(ui, self.storage_hidden, &t("Hidden files")).clicked() {
                    self.storage_hidden = !self.storage_hidden;
                }
                if kit::filled_button(ui, &format!("⬆ {}", t("Upload files…"))).clicked() {
                    self.pick(Pick::Upload(peer, self.storage_path.clone()));
                }
                if kit::tonal_button(ui, &format!("+ {}", t("New folder"))).clicked() {
                    self.new_folder_open = true;
                }
            });
        });
        ui.add_space(8.0);

        // The selected files and what to do with them.
        let picked: Vec<StorageEntry> = self
            .storage_entries
            .iter()
            .filter(|e| !e.is_dir && self.storage_selected.contains(&e.name))
            .cloned()
            .collect();
        if !picked.is_empty() {
            kit::tinted_card(ui, p.secondary_container, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let total: u64 = picked.iter().map(|e| e.size).sum();
                    let what = match picked.as_slice() {
                        [one] => format!("{} {}", file_icon(&one.name), one.name),
                        many => format!("{} {}", many.len(), t("files selected")),
                    };
                    ui.label(egui::RichText::new(what).strong());
                    ui.label(egui::RichText::new(human_size(total)).color(p.on_surface_variant));
                    if kit::filled_button(ui, &format!("⬇ {}", t("Download"))).clicked() {
                        for e in &picked {
                            let full = self.storage_join(&e.name);
                            self.download_from_phone(peer, full, e.name.clone(), e.size);
                        }
                        self.storage_selected.clear();
                    }
                    if kit::icon_button(ui, "✖", &t("Clear selection")).clicked() {
                        self.storage_selected.clear();
                    }
                });
            });
            ui.add_space(8.0);
        }

        // The contents, in their own card.
        let entries: Vec<StorageEntry> = self
            .storage_entries
            .iter()
            .filter(|e| self.storage_hidden || !e.name.starts_with('.'))
            .cloned()
            .collect();
        let pane = kit::glass(ui).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(ui.available_height() - 4.0);
                if let Some(err) = self.storage_error.clone() {
                    let (icon, why) = if err.contains("offline") {
                        ("📵", t("The phone is not connected. Check that both devices are on the same network."))
                    } else if err == broche_core::Error::TooLarge.to_string() {
                        ("📁", t("This folder contains too many entries to list."))
                    } else {
                        ("🔒", format!("{}\n\n{}", capitalized(&i18n::core_errors(&err)), t("Turn on \"Browse files between these devices\" in Settings › Device and allow it in the phone app.")))
                    };
                    kit::empty_state(ui, icon, &t("Can't open this folder"), &why);
                    ui.vertical_centered(|ui| {
                        if kit::tonal_button(ui, &t("Try again")).clicked() {
                            self.list_storage(self.storage_path.clone());
                        }
                    });
                    return;
                }
                if self.storage_loading && entries.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(24.0);
                        ui.spinner();
                    });
                    return;
                }
                if entries.is_empty() {
                    kit::empty_state(ui, "📂", &t("This folder is empty"), &t("Upload files from this PC with the button above."));
                    return;
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.storage_list {
                        self.storage_rows(ui, peer, &entries);
                    } else {
                        self.storage_grid(ui, peer, &entries);
                    }
                });
            });
        kit::glass_edge(ui, pane.response.rect);

        if self.new_folder_open {
            let mut close = false;
            egui::Modal::new(egui::Id::new("new_folder")).show(ui.ctx(), |ui| {
                ui.set_width(360.0);
                kit::title(ui, &t("New folder"));
                let r = kit::field(ui, &mut self.new_folder, &t("Folder name"), 0.0);
                r.request_focus();
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if kit::tonal_button(ui, &t("Cancel")).clicked() {
                        close = true;
                    }
                    if (kit::filled_button(ui, &t("Create")).clicked() || enter)
                        && !self.new_folder.trim().is_empty()
                    {
                        let full = self.storage_join(self.new_folder.trim());
                        let dir = self.storage_path.clone();
                        let tx = self.tx.clone();
                        self.act("mkdir", move |n| async move {
                            let r = n
                                .storage_mkdir(peer, full)
                                .await
                                .map(|_| t("Done."))
                                .map_err(|e| e.to_string());
                            let _ = tx.send(Msg::StorageRefresh(dir));
                            r
                        });
                        close = true;
                    }
                });
            });
            if close {
                self.new_folder_open = false;
                self.new_folder.clear();
            }
        }
    }

    /// Adds a file to the selection, or takes it out if it was in.
    fn toggle_storage_pick(&mut self, name: &str) {
        if !self.storage_selected.remove(name) {
            self.storage_selected.insert(name.to_string());
        }
    }

    fn storage_open(&mut self, peer: DeviceId, e: &StorageEntry) {
        if e.is_dir {
            self.list_storage(self.storage_join(&e.name));
        } else {
            let full = self.storage_join(&e.name);
            self.download_from_phone(peer, full, e.name.clone(), e.size);
        }
    }

    /// Folders and files as tiles, as many per row as fit.
    fn storage_grid(&mut self, ui: &mut egui::Ui, peer: DeviceId, entries: &[StorageEntry]) {
        let p = theme::pal(ui);
        let gap = 8.0;
        let (cols, w) = kit::columns_for(ui.available_width(), 128.0, gap, 16);
        for row in entries.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for e in row {
                    let selected = !e.is_dir && self.storage_selected.contains(&e.name);
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 124.0), egui::Sense::click());
                    let hot = resp.hovered() || resp.has_focus();
                    let painter = ui.painter();
                    painter.rect(
                        rect,
                        egui::CornerRadius::same(12),
                        if selected {
                            p.primary_container
                        } else if hot {
                            p.surface_container_high
                        } else {
                            p.surface_container
                        },
                        egui::Stroke::new(
                            if hot || selected { 2.0_f32 } else { 1.0 },
                            if hot || selected { p.primary } else { p.outline_variant },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    let icon = if e.is_dir { "📁" } else { file_icon(&e.name) };
                    painter.text(
                        egui::pos2(rect.center().x, rect.top() + 34.0),
                        egui::Align2::CENTER_CENTER,
                        icon,
                        egui::FontId::proportional(34.0),
                        if e.is_dir { p.primary } else { p.on_surface_variant },
                    );
                    let mut job = egui::text::LayoutJob::simple(
                        e.name.clone(),
                        egui::FontId::proportional(13.0),
                        if selected { p.on_primary_container } else { p.on_surface },
                        w - 16.0,
                    );
                    job.wrap.max_rows = 2;
                    job.wrap.break_anywhere = true;
                    job.halign = egui::Align::Center;
                    let galley = painter.layout_job(job);
                    painter.galley(egui::pos2(rect.center().x, rect.top() + 62.0), galley, p.on_surface);
                    if !e.is_dir {
                        painter.text(
                            egui::pos2(rect.center().x, rect.bottom() - 12.0),
                            egui::Align2::CENTER_CENTER,
                            human_size(e.size),
                            egui::FontId::proportional(11.5),
                            p.on_surface_variant,
                        );
                    }
                    let tip =
                        if e.is_dir { t("Open") } else { t("Click to select, double-click to download") };
                    let name = e.name.clone();
                    resp.widget_info(|| {
                        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name)
                    });
                    let resp = resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.double_clicked() && !e.is_dir {
                        self.storage_open(peer, e);
                    } else if resp.clicked() {
                        if e.is_dir {
                            self.storage_open(peer, e);
                        } else {
                            self.toggle_storage_pick(&e.name);
                        }
                    }
                    resp.context_menu(|ui| {
                        if ui.button(if e.is_dir { t("Open") } else { t("Download") }).clicked() {
                            self.storage_open(peer, e);
                            ui.close();
                        }
                    });
                }
            });
            ui.add_space(gap - ui.spacing().item_spacing.y);
        }
    }

    /// Folders and files as a list: icon, name, size and an action.
    fn storage_rows(&mut self, ui: &mut egui::Ui, peer: DeviceId, entries: &[StorageEntry]) {
        let p = theme::pal(ui);
        for (i, e) in entries.iter().enumerate() {
            if i > 0 {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
                ui.painter().rect_filled(rect.shrink2(egui::vec2(8.0, 0.0)), 0.0, p.outline_variant);
            }
            let (rect, resp) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::click());
            let hot = resp.hovered() || resp.has_focus();
            let selected = !e.is_dir && self.storage_selected.contains(&e.name);
            let painter = ui.painter();
            if selected {
                painter.rect_filled(rect, egui::CornerRadius::same(10), p.primary_container);
            } else if hot {
                painter.rect_filled(rect, egui::CornerRadius::same(10), p.surface_container_high);
            }
            if resp.has_focus() {
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::same(10),
                    egui::Stroke::new(2.0_f32, p.primary),
                    egui::StrokeKind::Inside,
                );
            }
            let icon = if e.is_dir { "📁" } else { file_icon(&e.name) };
            painter.text(
                egui::pos2(rect.left() + 24.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(20.0),
                if e.is_dir { p.primary } else { p.on_surface_variant },
            );
            let right = if e.is_dir { t("Folder") } else { human_size(e.size) };
            let right_w = 140.0;
            let galley = painter.layout(
                e.name.clone(),
                egui::FontId::proportional(14.5),
                p.on_surface,
                (rect.width() - 56.0 - right_w).max(40.0),
            );
            let galley = if galley.rows.len() > 1 {
                let mut job = egui::text::LayoutJob::simple_singleline(
                    e.name.clone(),
                    egui::FontId::proportional(14.5),
                    p.on_surface,
                );
                job.wrap.max_width = (rect.width() - 56.0 - right_w).max(40.0);
                job.wrap.max_rows = 1;
                painter.layout_job(job)
            } else {
                galley
            };
            painter.galley(
                egui::pos2(rect.left() + 48.0, rect.center().y - galley.size().y / 2.0),
                galley,
                p.on_surface,
            );
            painter.text(
                egui::pos2(rect.right() - 56.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                right,
                egui::FontId::proportional(13.0),
                p.on_surface_variant,
            );
            let name = e.name.clone();
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name));
            let tip = if e.is_dir { t("Open") } else { t("Click to select, double-click to download") };
            let resp = resp.on_hover_text(&tip).on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.double_clicked() && !e.is_dir {
                self.storage_open(peer, e);
            } else if resp.clicked() {
                if e.is_dir {
                    self.storage_open(peer, e);
                } else {
                    self.toggle_storage_pick(&e.name);
                }
            }
            // The action at the end of the row.
            let btn = egui::Rect::from_center_size(
                egui::pos2(rect.right() - 26.0, rect.center().y),
                egui::vec2(40.0, 40.0),
            );
            let icon = if e.is_dir { "›" } else { "⬇" };
            ui.painter().text(
                btn.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(18.0),
                p.primary,
            );
        }
    }

    fn set_toggle(&mut self, peer: DeviceId, id: &'static str, value: PhoneToggleValue) {
        self.act(id, move |n| async move {
            n.set_phone_toggle(peer, id.to_string(), value)
                .await
                .map(|_| t("Done."))
                .map_err(|e| e.to_string())
        });
    }

    /// Opens the desktop's file chooser (portal); the answer arrives as `Msg::Picked`.
    pub(crate) fn pick(&self, purpose: Pick) {
        let tx = self.tx.clone();
        let directory = matches!(purpose, Pick::SharedFolder);
        let title = t(match purpose {
            Pick::Send(_) => "Choose files to send",
            Pick::Upload(..) => "Choose files to upload to the phone",
            Pick::SharedFolder => "Choose shared folder",
        });
        self.rt.spawn(async move {
            use ashpd::desktop::file_chooser::{FileChooserProxy, OpenFileOptions};
            let result = async {
                let portal = FileChooserProxy::new().await?;
                let selected = portal
                    .open_file(
                        None,
                        &title,
                        OpenFileOptions::default().set_directory(directory).set_multiple(!directory),
                    )
                    .await?
                    .response()?;
                Ok::<_, ashpd::Error>(selected)
            }
            .await;
            match result {
                Ok(selected) => {
                    let paths: Vec<PathBuf> = selected
                        .uris()
                        .iter()
                        .filter_map(|u| url::Url::parse(u.as_str()).ok()?.to_file_path().ok())
                        .collect();
                    let _ = tx.send(Msg::Picked(purpose, paths));
                }
                // Cancelling a dialog is not an application error.
                Err(e) => eprintln!("File chooser: {e}"),
            }
        });
    }

    fn send_paths(&mut self, peer: DeviceId, paths: Vec<PathBuf>) {
        let label = format!("send {} item(s)", paths.len());
        self.act(&label, move |n| async move {
            let files =
                broche_core::outgoing_paths(&paths).map_err(|e| format!("can't read what to send: {e}"))?;
            n.send_files(peer, files).await.map(|_| t("Transfer started.")).map_err(|e| e.to_string())
        });
    }

    fn screen_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(_) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        let shared = self.platform.mirror.clone();
        let active = shared.active.load(Ordering::Acquire);
        if !active && !self.mirror_wanted {
            egui::ScrollArea::vertical().show(ui, |ui| {
                kit::card(ui, |ui| {
                    kit::title(ui, &t("Show the phone's screen"));
                    kit::supporting(ui, t("Click and drag on the picture to touch the phone's screen."));
                    ui.add_space(4.0);
                    // Next to its text: everything else in this card is aligned to the left.
                    ui.horizontal(|ui| {
                        let label = t("Also play the phone's sound");
                        kit::switch(ui, &mut self.mirror_sound, &label);
                        if ui.add(egui::Label::new(&label).sense(egui::Sense::click())).clicked() {
                            self.mirror_sound = !self.mirror_sound;
                        }
                    });
                    ui.label(t("The phone's own screen meanwhile"));
                    self.mirror_screen_choice(ui, false);
                    ui.add_space(4.0);
                    if kit::filled_button(ui, &t("Show the phone's screen")).clicked() {
                        self.start_mirror();
                    }
                });
            });
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if kit::danger_button(ui, &format!("■ {}", t("Stop"))).clicked() {
                self.stop_mirror();
            }
            if !self.phone_window
                && kit::tonal_button(ui, &format!("📱 {}", t("Show only the phone"))).clicked()
            {
                self.phone_window = true;
            }
            for (label, key) in [("◀", "back"), ("🏠", "home"), ("▣", "recents")] {
                let name = match key {
                    "back" => t("Back"),
                    "home" => t("Home"),
                    _ => t("Recents"),
                };
                if kit::tonal_button(ui, &format!("{label} {name}")).clicked()
                    && let Some(tx) = &self.mirror_input
                {
                    let _ = tx.send(MirrorInput::Key { key: key.into() });
                }
            }
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.mirror_text)
                    .hint_text(t("Type on the phone:"))
                    .desired_width(220.0)
                    .margin(egui::vec2(12.0, 10.0)),
            );
            let r = kit::named(r, &t("Type on the phone:"));
            if (kit::tonal_button(ui, &t("Send")).clicked()
                || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                && !self.mirror_text.is_empty()
                && let Some(tx) = &self.mirror_input
            {
                let _ = tx.send(MirrorInput::Text { text: std::mem::take(&mut self.mirror_text) });
            }
        });
        ui.label(t("The phone's own screen meanwhile"));
        self.mirror_screen_choice(ui, true);
        if !active {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(t("Accept the request on the phone."));
            });
        }
        self.pull_mirror_frame(ctx, active);
        if !active {
            return;
        }
        self.mirror_surface(ui);
    }

    /// Takes the newest decoded frame of the phone's screen into the texture both views draw.
    fn pull_mirror_frame(&mut self, ctx: &egui::Context, active: bool) {
        if let Some(img) = self.platform.mirror.frame.lock().unwrap().take() {
            match &mut self.mirror_tex {
                Some(tex) => tex.set(img, Default::default()),
                None => self.mirror_tex = Some(ctx.load_texture("mirror", img, Default::default())),
            }
        }
        if !active {
            self.mirror_tex = None;
        }
    }

    /// The phone's screen, as large as fits, taking clicks, drags and scrolling as touches.
    fn mirror_surface(&mut self, ui: &mut egui::Ui) {
        let Some(tex) = &self.mirror_tex else { return };
        if !ui.ctx().input(|i| i.viewport().minimized.unwrap_or(false)) {
            *self.platform.mirror.seen.lock().unwrap() = Some(std::time::Instant::now());
        }
        let tx = self.mirror_input.clone();
        apps::touch_surface(ui, tex, &mut self.touching, |input| {
            if let Some(tx) = &tx {
                let _ = tx.send(input);
            }
        });
    }

    fn messages_tab(&mut self, ui: &mut egui::Ui) {
        let Some(peer) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        let wide = ui.available_width() >= 760.0;
        let height = ui.available_height();
        if wide {
            ui.horizontal_top(|ui| {
                let list_w = (ui.available_width() * 0.36).clamp(260.0, 380.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(list_w, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.threads_pane(ui);
                    },
                );
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.conversation_pane(ui, peer, false);
                    },
                );
            });
        } else if self.open_thread.is_some() || self.composing {
            self.conversation_pane(ui, peer, true);
        } else {
            self.threads_pane(ui);
        }
    }

    fn threads_pane(&mut self, ui: &mut egui::Ui) {
        let p = theme::pal(ui);
        let pane = kit::glass(ui).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(ui.available_height() - 4.0);
            ui.horizontal(|ui| {
                if kit::filled_button(ui, &format!("✏ {}", t("New message"))).clicked() {
                    self.open_thread = None;
                    self.sms_messages.clear();
                    self.sms_to.clear();
                    self.composing = true;
                }
                if kit::icon_button(ui, "⟳", &t("Refresh")).clicked() {
                    self.load_threads();
                    if let Some(id) = self.open_thread.clone() {
                        self.load_thread(id);
                    }
                }
            });
            egui::ScrollArea::vertical().id_salt("threads").auto_shrink([false, false]).show(ui, |ui| {
                if self.threads.is_empty() {
                    kit::empty_state(ui, "💬", &t("No conversations."), "");
                }
                for th in self.threads.clone() {
                    let name = thread_name(&th);
                    let selected = self.open_thread.as_deref() == Some(th.id.as_str());
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(ui.available_width(), 60.0), egui::Sense::click());
                    let hot = resp.hovered() || resp.has_focus();
                    let painter = ui.painter();
                    if selected || hot {
                        painter.rect_filled(
                            rect,
                            egui::CornerRadius::same(12),
                            if selected { p.primary_container } else { p.surface_container_high },
                        );
                    }
                    if resp.has_focus() {
                        painter.rect_stroke(
                            rect,
                            egui::CornerRadius::same(12),
                            egui::Stroke::new(2.0_f32, p.primary),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
                    let avatar = egui::pos2(rect.left() + 26.0, rect.center().y);
                    painter.circle_filled(avatar, 18.0, p.secondary_container);
                    painter.text(
                        avatar,
                        egui::Align2::CENTER_CENTER,
                        initial,
                        egui::FontId::proportional(15.0),
                        p.on_secondary_container,
                    );
                    let fg = if selected { p.on_primary_container } else { p.on_surface };
                    let wrap = rect.width() - 64.0;
                    let mut title_job = egui::text::LayoutJob::simple_singleline(
                        if th.unread > 0 { format!("{name}  ({})", th.unread) } else { name.clone() },
                        egui::FontId::proportional(14.5),
                        fg,
                    );
                    title_job.wrap.max_width = wrap;
                    title_job.wrap.max_rows = 1;
                    let mut snip_job = egui::text::LayoutJob::simple_singleline(
                        th.snippet.replace('\n', " "),
                        egui::FontId::proportional(12.5),
                        p.on_surface_variant,
                    );
                    snip_job.wrap.max_width = wrap;
                    snip_job.wrap.max_rows = 1;
                    painter.galley(
                        egui::pos2(rect.left() + 52.0, rect.top() + 10.0),
                        painter.layout_job(title_job),
                        fg,
                    );
                    painter.galley(
                        egui::pos2(rect.left() + 52.0, rect.top() + 33.0),
                        painter.layout_job(snip_job),
                        p.on_surface_variant,
                    );
                    resp.widget_info(|| {
                        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &name)
                    });
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.open_thread = Some(th.id.clone());
                        self.composing = false;
                        self.sms_to = th.addresses.first().cloned().unwrap_or_default();
                        self.sms_messages.clear();
                        self.load_thread(th.id.clone());
                    }
                }
            });
        });
        kit::glass_edge(ui, pane.response.rect);
    }

    fn conversation_pane(&mut self, ui: &mut egui::Ui, peer: DeviceId, narrow: bool) {
        let p = theme::pal(ui);
        let pane = kit::glass(ui).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(ui.available_height() - 4.0);
            let thread =
                self.open_thread.as_ref().and_then(|id| self.threads.iter().find(|t| &t.id == id)).cloned();
            ui.horizontal(|ui| {
                match &thread {
                    Some(th) => kit::title(ui, &thread_name(th)),
                    None if self.composing || narrow => kit::title(ui, &format!("✏ {}", t("New message"))),
                    None => {}
                }
                // A clear way out: a labelled button, not just an arrow.
                let open = thread.is_some() || self.composing || narrow;
                if open {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if thread.is_none() {
                            format!("✖ {}", t("Cancel"))
                        } else if narrow {
                            format!("← {}", t("Conversations"))
                        } else {
                            format!("✖ {}", t("Close"))
                        };
                        if kit::tonal_button(ui, &label).clicked() {
                            self.open_thread = None;
                            self.composing = false;
                            self.sms_body.clear();
                        }
                    });
                }
            });
            if thread.is_none() {
                if !self.composing && !narrow {
                    kit::empty_state(ui, "💬", &t("Pick a conversation"), &t("Or write a new message."));
                    return;
                }
                kit::field(ui, &mut self.sms_to, &t("To (number):"), 0.0);
            }
            kit::divider(ui);
            let composer_h = 56.0;
            egui::ScrollArea::vertical()
                .id_salt("sms")
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .max_height((ui.available_height() - composer_h).max(80.0))
                .show(ui, |ui| {
                    let max_w = (ui.available_width() * 0.75).max(160.0);
                    for m in self.sms_messages.iter().rev() {
                        let layout = if m.outgoing {
                            egui::Layout::right_to_left(egui::Align::TOP)
                        } else {
                            egui::Layout::left_to_right(egui::Align::TOP)
                        };
                        ui.with_layout(layout, |ui| {
                            egui::Frame::new()
                                .fill(if m.outgoing { p.primary_container } else { p.surface_container_high })
                                .corner_radius(egui::CornerRadius::same(16))
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.set_max_width(max_w);
                                    ui.add(
                                        egui::Label::new(egui::RichText::new(&m.body).color(if m.outgoing {
                                            p.on_primary_container
                                        } else {
                                            p.on_surface
                                        }))
                                        .wrap(),
                                    );
                                });
                        });
                    }
                });
            {
                let (_, send, enter) =
                    kit::field_with_button(ui, &mut self.sms_body, &t("Message:"), &t("Send"), true);
                if (send || enter) && !self.sms_body.is_empty() && !self.sms_to.trim().is_empty() {
                    let (to, body) = (self.sms_to.trim().to_string(), std::mem::take(&mut self.sms_body));
                    self.act("sms", move |n| async move {
                        n.sms_send(peer, vec![to], body).await.map(|_| t("Done.")).map_err(|e| e.to_string())
                    });
                    if let Some(id) = self.open_thread.clone() {
                        self.load_thread(id);
                    }
                }
            }
        });
        kit::glass_edge(ui, pane.response.rect);
    }

    fn calls_tab(&mut self, ui: &mut egui::Ui) {
        let Some(peer) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            if let Some(call) = self.call.clone() {
                kit::tinted_card(ui, theme::pal(ui).primary_container, |ui| {
                    kit::title(ui, &t("Call in progress"));
                    let who = call.name.clone().or(call.number.clone()).unwrap_or_default();
                    ui.label(format!("{who} — {}", call.state));
                    ui.horizontal_wrapped(|ui| {
                        let ringing = call.state.contains("ring") && call.incoming;
                        if ringing {
                            self.call_button(ui, peer, &call, t("Answer"), CallCommand::Answer);
                            self.call_button(ui, peer, &call, t("Silence"), CallCommand::Silence);
                        } else if let Some(c) = &call.controls {
                            self.call_button(ui, peer, &call, t("Mute"), CallCommand::Mute(!c.muted));
                            self.call_button(ui, peer, &call, t("Speaker"), CallCommand::Speaker(!c.speaker));
                            if c.can_hold {
                                self.call_button(ui, peer, &call, t("Hold"), CallCommand::Hold(!c.held));
                            }
                        }
                        if call.state == "active"
                            && let Some(ag) =
                                bt_calls::audio_gateway(call.number.as_deref().unwrap_or_default())
                        {
                            let on_pc = ag.transport_state == "active";
                            if kit::named(
                                ui.add_enabled(
                                    !on_pc,
                                    egui::Button::new(if on_pc {
                                        t("Audio on this PC")
                                    } else {
                                        t("Talk on this PC")
                                    }),
                                ),
                                &t("Talk on this PC"),
                            )
                            .clicked()
                            {
                                self.act("call_audio", move |n| async move {
                                    bt_calls::activate_audio(&n, peer, &ag.path).await.map(|_| t("Done."))
                                });
                            }
                        }

                        let label = if ringing { t("Decline") } else { t("Hang up") };
                        if kit::danger_button(ui, &label).clicked() {
                            let call = call.clone();
                            self.act("call", move |n| async move {
                                bt_calls::dispatch(&n, peer, &call, CallCommand::Decline)
                                    .await
                                    .map(|_| t("Done."))
                                    .map_err(|e| e.to_string())
                            });
                        }
                    });
                });
            }
            let wide = ui.available_width() >= 760.0;
            if wide {
                ui.columns(2, |cols| {
                    cols[0].spacing_mut().item_spacing.y = 12.0;
                    self.dial_card(&mut cols[0], peer);
                    self.recent_calls_card(&mut cols[0], peer);
                    self.contacts_card(&mut cols[1], peer);
                });
            } else {
                self.dial_card(ui, peer);
                self.recent_calls_card(ui, peer);
                self.contacts_card(ui, peer);
            }
        });
    }

    fn dial_card(&mut self, ui: &mut egui::Ui, peer: DeviceId) {
        kit::card(ui, |ui| {
            kit::title(ui, &t("Dial a number"));
            let (r, call, enter) =
                kit::field_with_button(ui, &mut self.dial, "+505 …", &format!("📞 {}", t("Call")), true);
            kit::named(r, &t("Dial a number"));
            if (call || enter) && !self.dial.trim().is_empty() {
                let number = self.dial.trim().to_string();
                self.act("call", move |n| async move {
                    n.call_dial(peer, number).await.map(|_| t("Done.")).map_err(|e| e.to_string())
                });
            }
        });
    }

    fn recent_calls_card(&mut self, ui: &mut egui::Ui, peer: DeviceId) {
        kit::card(ui, |ui| {
            ui.horizontal(|ui| {
                kit::title(ui, &t("Recent calls"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if kit::icon_button(ui, "⟳", &t("Refresh")).clicked() {
                        self.load(move |n| async move {
                            n.call_log(peer, None, 40).await.map(Msg::CallLog).map_err(|e| e.to_string())
                        });
                    }
                });
            });
            if self.calls.is_empty() {
                kit::supporting(ui, t("No calls."));
            }
            for (i, c) in self.calls.clone().into_iter().enumerate() {
                if i > 0 {
                    kit::divider(ui);
                }
                let who = c.name.clone().unwrap_or_else(|| c.number.clone());
                let (dir, icon) = match c.direction.as_str() {
                    "incoming" | "in" => (t("Incoming"), "↙"),
                    "missed" => (t("Missed"), "✖"),
                    "outgoing" | "out" => (t("Outgoing"), "↗"),
                    "rejected" => (t("Declined"), "✖"),
                    other => (other.to_string(), "•"),
                };
                ui.horizontal(|ui| {
                    let p = theme::pal(ui);
                    ui.label(egui::RichText::new(icon).color(
                        if matches!(c.direction.as_str(), "missed" | "rejected") {
                            p.error
                        } else {
                            p.on_surface_variant
                        },
                    ));
                    ui.vertical(|ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&who).strong()).truncate());
                        ui.label(
                            egui::RichText::new(
                                [
                                    Some(dir),
                                    history::call_length(c.duration),
                                    Some(history::ago(history::chrono_now() - c.date / 1000)),
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" · "),
                            )
                            .small()
                            .color(p.on_surface_variant),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if kit::tonal_icon_button(
                            ui,
                            "📞",
                            &format!("{} {}", t("Put in the dial box:"), c.number),
                        )
                        .clicked()
                        {
                            self.dial = c.number.clone();
                            self.log.push(format!("{} {}", t("Number ready, press Call:"), c.number));
                        }
                    });
                });
            }
        });
    }

    fn contacts_card(&mut self, ui: &mut egui::Ui, peer: DeviceId) {
        kit::card(ui, |ui| {
            kit::title(ui, &t("Contacts"));
            let (_, search, enter) = kit::field_with_button(
                ui,
                &mut self.contact_query,
                &t("Search"),
                &format!("🔍 {}", t("Search")),
                false,
            );
            if search || enter {
                let q = Some(self.contact_query.trim().to_string()).filter(|q| !q.is_empty());
                self.load(move |n| async move {
                    n.contacts(peer, q, 0, 50).await.map(Msg::Contacts).map_err(|e| e.to_string())
                });
            }
            if self.contacts.is_empty() {
                kit::supporting(ui, t("No contacts."));
            }
            let mut first = true;
            for c in self.contacts.clone() {
                for num in &c.numbers {
                    if !first {
                        kit::divider(ui);
                    }
                    first = false;
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{}{}",
                                        if c.starred { "★ " } else { "" },
                                        c.name
                                    ))
                                    .strong(),
                                )
                                .truncate(),
                            );
                            ui.label(
                                egui::RichText::new(&num.number)
                                    .small()
                                    .color(theme::pal(ui).on_surface_variant),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if kit::tonal_icon_button(ui, "💬", &t("Write a message")).clicked() {
                                self.sms_to = num.number.clone();
                                self.open_thread = None;
                                self.composing = true;
                                self.go_tab(Tab::Messages);
                            }
                            if kit::tonal_icon_button(
                                ui,
                                "📞",
                                &format!("{} {}", t("Put in the dial box:"), num.number),
                            )
                            .clicked()
                            {
                                self.dial = num.number.clone();
                                self.log.push(format!("{} {}", t("Number ready, press Call:"), num.number));
                            }
                        });
                    });
                }
            }
        });
    }

    fn call_button(
        &mut self,
        ui: &mut egui::Ui,
        peer: DeviceId,
        call: &CallState,
        label: String,
        cmd: CallCommand,
    ) {
        if kit::tonal_button(ui, &label).clicked() {
            let call = call.clone();
            self.act("call", move |n| async move {
                bt_calls::dispatch(&n, peer, &call, cmd).await.map(|_| t("Done.")).map_err(|e| e.to_string())
            });
        }
    }

    fn notifications_tab(&mut self, ui: &mut egui::Ui) {
        let Some(peer) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        if self.notifications.is_empty() {
            kit::card(ui, |ui| {
                kit::empty_state(
                    ui,
                    "🔔",
                    &t("No notifications."),
                    &t("New notifications from the phone appear here."),
                )
            });
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            for n in self.notifications.clone() {
                kit::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&n.app_name).small().strong().color(theme::pal(ui).primary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if kit::icon_button(ui, "✖", &t("Dismiss")).clicked() {
                                let key = n.key.clone();
                                self.act("dismiss", move |node| async move {
                                    node.dismiss_notification(peer, key)
                                        .await
                                        .map(|_| t("Done."))
                                        .map_err(|e| e.to_string())
                                });
                            }
                        });
                    });
                    if let Some(title) = &n.title {
                        ui.add(egui::Label::new(egui::RichText::new(title).strong()).wrap());
                    }
                    if let Some(text) = &n.text {
                        ui.add(egui::Label::new(text).wrap());
                    }
                    if n.actions.is_empty() {
                        return;
                    }
                    ui.horizontal_wrapped(|ui| {
                        for a in &n.actions {
                            if a.reply {
                                let text = self.reply_text.entry(format!("{}:{}", n.key, a.id)).or_default();
                                let r = ui.add(
                                    egui::TextEdit::singleline(text)
                                        .hint_text(t("Reply"))
                                        .desired_width(220.0)
                                        .margin(egui::vec2(12.0, 10.0)),
                                );
                                kit::named(r, &t("Reply"));
                                if kit::filled_button(ui, &a.title).clicked() {
                                    let reply = std::mem::take(text);
                                    if !reply.is_empty() {
                                        let (key, action) = (n.key.clone(), a.id.clone());
                                        self.act("reply", move |node| async move {
                                            node.run_notification_action(peer, key, action, Some(reply))
                                                .await
                                                .map(|_| t("Done."))
                                                .map_err(|e| e.to_string())
                                        });
                                    }
                                }
                            } else if kit::tonal_button(ui, &a.title).clicked() {
                                let (key, action) = (n.key.clone(), a.id.clone());
                                self.act("action", move |node| async move {
                                    node.run_notification_action(peer, key, action, None)
                                        .await
                                        .map(|_| t("Done."))
                                        .map_err(|e| e.to_string())
                                });
                            }
                        }
                    });
                });
            }
        });
    }

    fn load_photos(&mut self, peer: DeviceId) {
        let album = self.album.clone();
        self.thumbs.clear();
        self.photos_requested = true;
        self.load(move |n| async move {
            n.photo_list(peer, album, None, 60).await.map(Msg::PhotoItems).map_err(|e| e.to_string())
        });
    }

    fn photos_tab(&mut self, ui: &mut egui::Ui) {
        let Some(peer) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        if !self.photos_requested {
            self.load_photos(peer);
        }
        ui.horizontal_wrapped(|ui| {
            let current = self
                .album
                .as_ref()
                .and_then(|id| self.albums.iter().find(|a| &a.id == id))
                .map_or_else(|| t("All photos"), |a| a.name.clone());
            let before = self.album.clone();
            let combo =
                egui::ComboBox::from_id_salt("album").selected_text(current).width(220.0).show_ui(ui, |ui| {
                    if ui.selectable_label(self.album.is_none(), t("All photos")).clicked() {
                        self.album = None;
                    }
                    for a in self.albums.clone() {
                        if ui
                            .selectable_label(
                                self.album.as_ref() == Some(&a.id),
                                format!("{} ({})", a.name, a.count),
                            )
                            .clicked()
                        {
                            self.album = Some(a.id.clone());
                        }
                    }
                });
            kit::named(combo.response, &t("Photos"));
            if before != self.album {
                self.load_photos(peer);
            }
            if kit::icon_button(ui, "⟳", &t("Refresh")).clicked() {
                self.load(move |n| async move {
                    n.photo_albums(peer).await.map(Msg::Albums).map_err(|e| e.to_string())
                });
                self.load_photos(peer);
            }
            kit::supporting(ui, t("Click a photo to download it to this PC."));
        });
        ui.add_space(8.0);
        if self.photos.is_empty() {
            kit::card(ui, |ui| {
                kit::empty_state(
                    ui,
                    "🖼",
                    &t("No photos."),
                    &t("Allow photos in the phone app, then refresh."),
                )
            });
            return;
        }
        let p = theme::pal(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let gap = 8.0;
            // Leave room for the scroll bar so the last column never spills over.
            let width = ui.available_width() - 4.0;
            let (cols, w) = kit::columns_for(width, 150.0, gap, 12);
            for row in self.photos.clone().chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for photo in row {
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, w), egui::Sense::click());
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, egui::CornerRadius::same(12), p.surface_container_high);
                        if let Some(tex) = self.thumbs.get(&photo.id) {
                            // Fill the square and crop the rest (like a gallery), never overflow.
                            let s = tex.size_vec2();
                            let (u, v) = if s.x > s.y {
                                ((1.0 - s.y / s.x) / 2.0, 0.0)
                            } else {
                                (0.0, (1.0 - s.x / s.y.max(1.0)) / 2.0)
                            };
                            let uv = egui::Rect::from_min_max(egui::pos2(u, v), egui::pos2(1.0 - u, 1.0 - v));
                            egui::Image::new(tex).uv(uv).corner_radius(12).paint_at(ui, rect);
                        } else {
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "🖼",
                                egui::FontId::proportional(28.0),
                                p.on_surface_variant,
                            );
                        }
                        let hot = resp.hovered() || resp.has_focus();
                        if hot {
                            let bar = egui::Rect::from_min_max(
                                egui::pos2(rect.left(), rect.bottom() - 36.0),
                                rect.max,
                            );
                            painter.rect_filled(
                                bar,
                                egui::CornerRadius { nw: 0, ne: 0, sw: 12, se: 12 },
                                egui::Color32::from_black_alpha(160),
                            );
                            painter.text(
                                bar.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("⬇ {}", t("Download")),
                                egui::FontId::proportional(13.5),
                                egui::Color32::WHITE,
                            );
                            painter.rect_stroke(
                                rect,
                                egui::CornerRadius::same(12),
                                egui::Stroke::new(3.0_f32, p.primary),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let name = format!("{} {}", t("Photo"), photo.id);
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            let id = photo.id.clone();
                            self.act("download", move |n| async move {
                                n.fetch_photo(peer, id)
                                    .await
                                    .map(|_| t("Transfer started."))
                                    .map_err(|e| e.to_string())
                            });
                        }
                    }
                });
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }
        });
    }

    /// Show a QR code, wait for the phone and confirm the matching code.
    fn pairing_card(&mut self, ui: &mut egui::Ui) {
        kit::card(ui, |ui| {
            kit::title(ui, &t("Pair a new device"));
            kit::supporting(ui, t("Open Broche on your phone and scan the code that appears here."));
            ui.horizontal(|ui| {
                if self.pair_task.is_none() {
                    if kit::filled_button(ui, &t("Show pairing code")).clicked() {
                        self.start_pairing();
                    }
                } else {
                    if kit::tonal_button(ui, &t("Cancel")).clicked() {
                        if let Some(task) = self.pair_task.take() {
                            task.abort();
                        }
                        if let Some(n) = &self.node {
                            n.pairing_cancel();
                        }
                        self.pair_link = None;
                        self.sas = None;
                    }
                    ui.spinner();
                    ui.label(t("Waiting for a device…"));
                }
            });
            if let Some(code) = self.sas.clone() {
                kit::tinted_card(ui, theme::pal(ui).primary_container, |ui| {
                    ui.label(t("Does the other device show this code?"));
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            &code[..3.min(code.len())],
                            code.get(3..).unwrap_or("")
                        ))
                        .size(30.0)
                        .strong()
                        .monospace(),
                    );
                    ui.horizontal(|ui| {
                        if kit::filled_button(ui, &t("Yes, it matches")).clicked() {
                            if let Some(n) = &self.node {
                                let _ = n.pairing_confirm(true);
                            }
                            self.sas = None;
                        }
                        if kit::danger_button(ui, &t("No")).clicked() {
                            if let Some(n) = &self.node {
                                let _ = n.pairing_confirm(false);
                            }
                            self.sas = None;
                        }
                    });
                });
            }
            if let Some(link) = self.pair_link.clone() {
                ui.label(t("Scan this code in the Broche app:"));
                draw_qr(ui, &link);
                if kit::tonal_button(ui, &t("Copy link")).clicked() {
                    ui.ctx().copy_text(link);
                }
            }
        });
    }

    fn pair_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 12.0;
        if let Some((peer, name, mut chosen)) = self.profile_prompt.clone() {
            let mut done = false;
            kit::card(ui, |ui| {
                kit::title(ui, &format!("{} {name}", t("What may this phone do? ·")));
                kit::supporting(ui, t("You can change it later in Settings › Device."));
                if let Some(p) = profile_choices(ui, Some(chosen)) {
                    chosen = p;
                }
                if kit::filled_button(ui, &t("Save")).clicked() {
                    if let Some(node) = &self.node
                        && let Err(e) = node.apply_permission_profile(peer, chosen)
                    {
                        self.log.push(e.to_string());
                    }
                    self.perm_cache = None;
                    done = true;
                }
            });
            self.profile_prompt = if done { None } else { Some((peer, name, chosen)) };
        }
        self.pairing_card(ui);
    }

    /// The technical log (what the status bar showed), for diagnosing problems.
    fn log_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if kit::tonal_button(ui, &t("Clear")).clicked() {
                self.log.clear();
            }
            if kit::tonal_button(ui, &t("Copy")).clicked() {
                ui.ctx().copy_text(self.log.join("\n"));
            }
        });
        egui::ScrollArea::vertical().stick_to_bottom(true).max_height(320.0).auto_shrink([false, true]).show(
            ui,
            |ui| {
                if self.log.is_empty() {
                    ui.label(t("No entries yet. Warnings and errors show up here."));
                }
                for l in &self.log {
                    ui.add(egui::Label::new(egui::RichText::new(l).monospace()).wrap());
                }
            },
        );
    }
}

/// Sends the core's warnings and errors (`BROCHE_LOG`, an `EnvFilter`, to change the level) to
/// the technical log and to stderr, which is the system log. Without this they were lost.
fn log_core_to(tx: mpsc::UnboundedSender<Msg>) {
    struct Lines(mpsc::UnboundedSender<Msg>);
    impl std::io::Write for Lines {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let _ = std::io::stderr().write_all(buf);
            let text = String::from_utf8_lossy(buf);
            if !text.trim().is_empty() {
                let _ = self.0.send(Msg::Line(text.trim_end().to_string()));
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    // zbus warns when a portal object has no properties to cache: harmless, so only its errors.
    let filter = tracing_subscriber::EnvFilter::try_from_env("BROCHE_LOG")
        .unwrap_or_else(|_| "warn,zbus=error".into());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(move || Lines(tx.clone()))
        .try_init();
}

/// A file name from another device made safe for this PC: no folders, no `..`, no
/// control characters, not hidden, not empty.
fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let clean: String = base.chars().filter(|c| !c.is_control()).collect();
    let clean = clean.trim().trim_start_matches('.').to_string();
    if clean.is_empty() { "file".into() } else { clean.chars().take(200).collect() }
}

/// `dir/name`, or `dir/name (2).ext`… if that already exists (never overwrites).
fn unique_path(dir: &std::path::Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    (2..10_000).map(|i| dir.join(format!("{stem} ({i}){ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

/// Who a conversation is with: names, else numbers, else "Unknown".
fn thread_name(th: &SmsThread) -> String {
    let pick =
        |v: &[String]| v.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
    let names = pick(&th.names);
    if !names.is_empty() {
        return names;
    }
    let numbers = pick(&th.addresses);
    if numbers.is_empty() { t("Unknown") } else { numbers }
}

/// A symbol for a file, from its extension.
fn file_icon(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "bmp" | "svg" => "🖼",
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "3gp" => "🎞",
        "mp3" | "ogg" | "opus" | "flac" | "wav" | "m4a" | "aac" => "🎵",
        "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "zst" => "📦",
        "apk" => "📲",
        "pdf" => "📕",
        "txt" | "md" | "doc" | "docx" | "odt" | "rtf" => "📝",
        _ => "📄",
    }
}

/// `1536` → `1,5 KB` (decimal comma in Spanish).
pub(crate) fn human_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u + 1 < units.len() {
        v /= 1024.0;
        u += 1;
    }
    let s = if u == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", units[u]) };
    if i18n::get() == Lang::Es { s.replace('.', ",") } else { s }
}

/// Permissions by group, as Settings › Permissions shows them (and its search finds them).
/// The permission profiles as a list to pick from, each with what it allows. Returns the one
/// clicked. `current` is marked; `None` (custom) marks none.
pub(crate) fn profile_choices(
    ui: &mut egui::Ui,
    current: Option<PermissionProfile>,
) -> Option<PermissionProfile> {
    let mut clicked = None;
    for profile in PermissionProfile::ALL {
        let (name, about) = match profile {
            PermissionProfile::NotificationsOnly => {
                (t("Notifications only"), t("Notifications, messages, calls and contacts. Nothing else."))
            }
            PermissionProfile::Basic => (
                t("Basic"),
                t(
                    "Everyday sharing: files, clipboard, photos, music, and the phone's screen and camera. Nothing that controls this PC.",
                ),
            ),
            PermissionProfile::Multimedia => (
                t("Multimedia"),
                t("Basic, plus browsing the phone's storage and seeing this PC's screen on the phone."),
            ),
            PermissionProfile::Full => (
                t("Full control"),
                t(
                    "Everything, including this PC's mouse, keyboard and shortcut commands. Only for your own phone.",
                ),
            ),
        };
        if ui.radio(current == Some(profile), &name).clicked() {
            clicked = Some(profile);
        }
        ui.indent(&name, |ui| kit::supporting(ui, about));
    }
    clicked
}

pub(crate) const PERM_GROUPS: [(&str, &str, &[&str]); 4] = [
    (
        "Communication",
        "See and answer the phone's notifications, messages and calls here.",
        &["notifications", "messages", "calls", "contacts"],
    ),
    (
        "Files and clipboard",
        "Move files, photos and copied text between both devices.",
        &["files", "clipboard", "photos", "storage", "remote_files"],
    ),
    (
        "Screen and control",
        "Show screens and cameras, and let the phone use this PC's mouse and keyboard.",
        &["mirroring", "webcam", "pc_mirror", "remote_input"],
    ),
    (
        "Other permissions",
        "Media, quick settings and actions on this PC.",
        &["media", "toggles", "pc_actions", "commands"],
    ),
];

/// Saves a webcam frame, from the stream already showing, as a PNG in the received folder.
fn save_frame(img: &egui::ColorImage, tx: mpsc::UnboundedSender<Msg>) {
    let (w, h) = (img.width() as u32, img.height() as u32);
    let pixels = img.as_raw().to_vec();
    std::thread::spawn(move || {
        let secs =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = std::fs::create_dir_all(received_dir());
        let path = received_dir().join(format!("webcam-{secs}.png"));
        let msg = match image::save_buffer(&path, &pixels, w, h, image::ColorType::Rgba8) {
            Ok(()) => Msg::Captured(path),
            Err(e) => Msg::Line(format!("{} {e}", t("Couldn't save the photo:"))),
        };
        let _ = tx.send(msg);
    });
}

/// `player`: the phone's player when already known; otherwise waits for the phone to name it.
async fn media(
    n: Node,
    peer: DeviceId,
    action: MediaAction,
    player: Option<String>,
) -> Result<String, String> {
    if let Some(player) = player {
        n.media_command(peer, player, action, None).await.map_err(|e| e.to_string())?;
        return Ok(t("Done."));
    }
    let mut events = n.events();
    n.set_device_toggle(peer, "media", true).map_err(|e| e.to_string())?;
    let wait = async {
        loop {
            if let Ok(NodeEvent::MediaChanged { device, players }) = events.recv().await
                && device == peer
                && let Some(p) = players.first()
            {
                return p.id.clone();
            }
        }
    };
    let player = tokio::time::timeout(Duration::from_secs(10), wait)
        .await
        .map_err(|_| t("nothing is playing on that device"))?;
    n.media_command(peer, player.clone(), action, None).await.map_err(|e| e.to_string())?;
    Ok(format!("{} ({player})", t("Done.")))
}

fn draw_qr(ui: &mut egui::Ui, data: &str) {
    let Ok(code) = qrcode::QrCode::new(data.as_bytes()) else {
        ui.label("(cannot draw QR)");
        return;
    };
    let n = code.width();
    let cell = 5.0;
    let quiet = 3.0;
    let size = (n as f32 + quiet * 2.0) * cell;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, egui::Color32::WHITE);
    for (i, c) in code.to_colors().iter().enumerate() {
        if *c == qrcode::Color::Dark {
            let (x, y) = ((i % n) as f32, (i / n) as f32);
            let min = rect.min + egui::vec2((x + quiet) * cell, (y + quiet) * cell);
            p.rect_filled(egui::Rect::from_min_size(min, egui::vec2(cell, cell)), 0.0, egui::Color32::BLACK);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.platform.mirror.window.load(Ordering::Relaxed) == 0 {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let id = match frame.window_handle().map(|h| h.as_raw()) {
                Ok(RawWindowHandle::Xlib(h)) => h.window as u32,
                Ok(RawWindowHandle::Xcb(h)) => h.window.get(),
                _ => 0,
            };
            self.platform.mirror.window.store(id, Ordering::Relaxed);
        }
        // Wayland can't hide a window (winit ignores it), so "keep running" minimizes it;
        // under X11 (XWayland mode) the window really goes to the tray.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quitting
            && !self.close_quits
            && self.shot_dir.is_none()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.on_x11 && self.tray_available.load(Ordering::Relaxed) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        }
        if std::mem::take(&mut self.start_minimized) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        self.drain(ctx);
        // The phone stopped sharing (from its notification, say): forget the request so no page
        // keeps waiting for it. Checked every frame: in "phone only" mode or on another page the
        // Screen page isn't drawn.
        let active = self.platform.mirror.active.load(Ordering::Acquire);
        if self.mirror_was_active && !active {
            self.mirror_wanted = false;
            self.mirror_input = None;
            self.mirror_tex = None;
        }
        self.mirror_was_active = active;
        {
            let selected = self.devices.iter().find(|d| Some(&d.id) == self.selected.as_ref());
            let mut state = self.tray_state.write().unwrap();
            state.name = selected.map(|d| d.name.clone()).unwrap_or_default();
            state.online = selected.is_some_and(|d| d.state.starts_with("online"));
            state.speaker = self.speaker;
            state.phone_playing = self.peer().and_then(|peer| {
                Some(self.players.get(&peer.to_string())?.iter().find(|p| p.playing)?.id.clone())
            });
            state.dnd = self.peer().and_then(|p| self.node.as_ref()?.phone_toggles(p)).map(|tg| tg.dnd);
            state.autostart = self.autostart_on;
            state.peer = self.peer();
            // Use the guard already held: phone_ringing() would re-lock this RwLock.
            state.ringing = state.ring_started.is_some_and(|t| t.elapsed() < Duration::from_secs(30));
            state.screen_on = self.platform.mirror.active.load(Ordering::Acquire) || self.mirror_wanted;
        }
        self.maintain_streams();
        self.autostart(ctx);
        ctx.request_repaint_after(Duration::from_millis(
            if self.platform.mirror.active.load(Ordering::Acquire) { 33 } else { 500 },
        ));
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty()
            && let Some(peer) = self.peer()
        {
            self.send_paths(peer, dropped);
        }

        if self.welcome_step.is_some() {
            self.welcome(ctx);
        } else if !self.phone_window(ctx) {
            self.palette_shortcut(ctx);
            self.shell(ctx);
            self.command_palette(ctx);
        }
        self.app_windows(ctx);
        self.shelf_window(ctx);
        self.island_window(ctx);
        if let Some(peer) = self.confirm_unpair {
            egui::Modal::new(egui::Id::new("confirm_unpair")).show(ctx, |ui| {
                ui.heading(t("Unpair this device"));
                ui.label(t("You will need to pair again to reconnect."));
                ui.horizontal(|ui| {
                    if ui.button(t("Cancel")).clicked() {
                        self.confirm_unpair = None;
                    }
                    if ui.button(t("Unpair this device")).clicked() {
                        self.act("unpair", move |n| async move {
                            n.unpair(peer).await.map(|_| t("Unpaired.")).map_err(|e| e.to_string())
                        });
                        self.confirm_unpair = None;
                    }
                });
            });
        }
        self.shots(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.platform.shutdown();
        if let (Some(node), Some(peer)) = (self.node.clone(), self.peer()) {
            self.rt.block_on(node.webcam_stop(peer));
        }
        if self.platform.mirror.active.load(Ordering::Acquire)
            && let (Some(node), Some(peer)) = (self.node.clone(), self.peer())
        {
            self.rt.block_on(node.mirror_stop(peer, 0));
        }
        if let Some(node) = self.node.clone() {
            self.rt.block_on(node.shutdown());
        }
    }
}

fn main() -> eframe::Result {
    // Broche always uses XWayland: without WAYLAND_DISPLAY winit opens an X11 window, which can
    // come to the front from the tray, hide in it, take dropped files and watch the clipboard.
    // Plain Wayland is only the fallback when no X server is reachable.
    if std::env::var_os("DISPLAY").is_some() {
        // SAFETY: first thing in main, before any thread is started.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
    }
    // One Broche per data folder: two with the same identity keep taking the phone's connection
    // from each other. Opening it again shows the window of the one already running (it may be
    // hidden in the tray). Not for the screenshot aid, which runs next to the real one.
    let mut already = None;
    if let Some(dir) = data_dir().filter(|_| std::env::var_os("BROCHE_SHOT").is_none()) {
        // By a relative name from inside the folder: a socket's path can't be longer than 108
        // bytes, and a data folder's can.
        let (sock, back) = ("instance.sock", std::env::current_dir());
        let _ = std::fs::create_dir_all(&dir);
        if std::env::set_current_dir(&dir).is_ok() {
            if std::os::unix::net::UnixStream::connect(sock).is_ok() {
                eprintln!("Broche is already running: showing its window.");
                return Ok(());
            }
            // Left by one that didn't exit cleanly.
            let _ = std::fs::remove_file(sock);
            already = std::os::unix::net::UnixListener::bind(sock).ok();
            if let Ok(back) = back {
                let _ = std::env::set_current_dir(back);
            }
        }
    }
    let icon = image::load_from_memory(include_bytes!("../data/icon-256.png")).ok().map(|img| {
        let rgba = img.to_rgba8();
        egui::IconData { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() }
    });
    // Testing aid: `BROCHE_SHOT_SIZE=600x700` opens the window at that size.
    let size = std::env::var("BROCHE_SHOT_SIZE")
        .ok()
        .and_then(|v| {
            v.split_once('x').and_then(|(w, h)| Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?]))
        })
        .unwrap_or([980.0, 740.0]);
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(size)
        .with_min_inner_size([420.0, 480.0])
        .with_title("Broche")
        .with_app_id("io.github.robertcjla.Broche");
    if let Some(icon) = icon {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    // No vsync: presenting a frame of a minimized window waits, and every window waits with it,
    // so a phone app's window crawled at a few frames a second while Broche was minimized.
    let options = eframe::NativeOptions { viewport, vsync: false, ..Default::default() };
    eframe::run_native(
        "Broche",
        options,
        Box::new(move |cc| {
            let app = App::new(cc);
            if let Some(listener) = already {
                let (tx, ctx) = (app.tx.clone(), cc.egui_ctx.clone());
                std::thread::spawn(move || {
                    for _ in listener.incoming() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        let _ = tx.send(Msg::ShowWindow);
                        ctx.request_repaint();
                    }
                });
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_mode_does_not_discover_or_pair_real_devices() {
        let info = DeviceInfo {
            name: "Preview PC".into(),
            kind: DeviceKind::Desktop,
            os: "linux".into(),
            os_ver: String::new(),
            model: None,
            accent: None,
            screen: None,
            install_id: None,
        };
        let mut config = NodeConfig::new(PathBuf::from("preview-test"), info, "0.2.0");
        configure_discovery(&mut config, true);
        assert!(config.lan_discovery);
        assert!(config.adb_server.is_none());
        configure_discovery(&mut config, false);
        assert!(config.lan_discovery);
        assert_eq!(config.adb_server, Some(broche_core::usb::DEFAULT_ADB_ADDR));
    }

    #[test]
    fn failed_upload_ignores_late_start_but_accepts_its_final_byte_count() {
        let mut files = FileTransfers::default();
        let mut island = island::Transfers::default();
        let mut first = broche_core::Transfer {
            id: "first-upload".into(),
            device: DeviceId([7; 32]),
            direction: broche_core::Direction::Outgoing,
            names: vec!["first.bin".into()],
            files: 1,
            total: 100,
            done: 0,
            state: TransferState::Waiting,
            open_on_arrival: false,
        };
        let mut second = first.clone();
        second.id = "second-upload".into();
        second.total = 900;
        for transfer in [&first, &second] {
            files.update(transfer);
            island.update(transfer);
        }
        first.done = 100;
        first.state = TransferState::Done { saved: vec![] };
        files.update(&first);
        island.update(&first);
        let failed =
            files.fail_upload(second.device, &second.id, "example failure".into()).expect("queued upload");
        files.update(&failed);
        island.update(&failed);
        assert!(!files.accept_report(&second));
        let mut incoming = second.clone();
        incoming.direction = broche_core::Direction::Incoming;
        incoming.state = TransferState::Done { saved: vec![] };
        assert!(files.accept_report(&incoming));
        assert!(!files.accept_report(&second));
        second.done = 40;
        second.state = TransferState::Failed(broche_core::TransferFailure::Unreachable);
        assert!(files.accept_report(&second));
        files.update(&second);
        island.update(&second);
        let Some(island::Activity::Transfer { done, total, state, .. }) = island.activity(15) else {
            panic!("whole group")
        };
        assert_eq!((done, total), (140, 1000));
        assert!(matches!(state, TransferState::Failed(_)));
        second.state = TransferState::Waiting;
        assert!(files.accept_report(&second));
    }

    #[test]
    fn file_progress_keeps_outcomes_and_tracks_both_directions_and_retries() {
        let mut progress = FileTransfers::default();
        let mut transfer = broche_core::Transfer {
            id: "example-transfer".into(),
            device: DeviceId([7; 32]),
            direction: broche_core::Direction::Outgoing,
            names: vec!["example.bin".into()],
            files: 1,
            total: 100,
            done: 0,
            state: TransferState::Waiting,
            open_on_arrival: false,
        };
        progress.update(&transfer);
        transfer.state = TransferState::Running;
        transfer.done = 40;
        progress.update(&transfer);
        assert_eq!(progress.items.len(), 1);
        assert_eq!(progress.items[0].done, 40);
        transfer.state = TransferState::Failed(broche_core::TransferFailure::Unreachable);
        progress.update(&transfer);
        assert!(progress.items[0].state.is_finished());
        transfer.state = TransferState::Paused;
        progress.update(&transfer);
        assert!(!progress.items[0].state.is_finished());
        let mut incoming = transfer.clone();
        incoming.direction = broche_core::Direction::Incoming;
        progress.update(&incoming);
        assert_eq!(progress.items.len(), 2);
        transfer.state = TransferState::Done { saved: vec![] };
        incoming.state = TransferState::Cancelled;
        progress.update(&transfer);
        progress.update(&incoming);
        assert_eq!(progress.items.len(), 2);
        let mut next = transfer.clone();
        next.id = "next-transfer".into();
        next.state = TransferState::Requested;
        next.done = 0;
        progress.update(&next);
        assert_eq!(progress.items.len(), 3);
    }

    #[test]
    fn next_upload_does_not_erase_a_failed_file() {
        let mut progress = FileTransfers::default();
        let mut transfer = broche_core::Transfer {
            id: "failed-file".into(),
            device: DeviceId([7; 32]),
            direction: broche_core::Direction::Outgoing,
            names: vec!["first.bin".into()],
            files: 1,
            total: 100,
            done: 40,
            state: TransferState::Failed(broche_core::TransferFailure::Unreachable),
            open_on_arrival: false,
        };
        progress.update(&transfer);
        transfer.id = "next-file".into();
        transfer.names = vec!["second.bin".into()];
        transfer.done = 0;
        transfer.state = TransferState::Waiting;
        progress.update(&transfer);
        assert_eq!(progress.items.len(), 2);
        assert!(matches!(progress.items[0].state, TransferState::Failed(_)));
        assert_eq!(progress.items[1].state, TransferState::Waiting);
    }

    #[test]
    fn download_batch_tracks_bytes_failures_and_new_selections() {
        let mut batch = Downloads::default();
        let small = batch.add("small.txt".into(), 10, DeviceId([7; 32]));
        let large = batch.add("large.bin".into(), 90, DeviceId([7; 32]));
        batch.progress(small, 10, 10);
        batch.finish(small, Ok(()));
        batch.progress(large, 20, 100);
        assert_eq!(batch.bytes(), (30, 110));
        assert_eq!(batch.completed(), 1);
        assert!(!batch.finished());
        let empty = batch.add("empty.txt".into(), 0, DeviceId([7; 32]));
        batch.finish(large, Err("connection lost".into()));
        assert!(!batch.finished());
        batch.finish(empty, Ok(()));
        assert!(batch.finished());
        assert_eq!(batch.completed(), 2);
        assert_eq!(batch.failed(), 1);
        assert_eq!(batch.bytes(), (30, 110));
        assert_eq!(batch.add("next.txt".into(), 4, DeviceId([7; 32])), 0);
        assert_eq!(batch.items.len(), 1);
        assert_eq!(batch.bytes(), (0, 4));
        assert!(!batch.finished());
    }

    #[test]
    fn empty_download_finishes_without_dividing_by_zero() {
        let mut batch = Downloads::default();
        let id = batch.add("empty.txt".into(), 0, DeviceId([7; 32]));
        assert_eq!(batch.fraction(), 0.0);
        batch.finish(id, Ok(()));
        assert_eq!(batch.fraction(), 1.0);
        assert!(batch.finished());
        assert_eq!(batch.completed(), 1);
    }

    #[test]
    fn pretty_hostname_from_machine_info() {
        assert_eq!(
            pretty_hostname("PRETTY_HOSTNAME=\"PC de Ana\"\nCHASSIS=laptop\n").as_deref(),
            Some("PC de Ana")
        );
        assert_eq!(pretty_hostname("PRETTY_HOSTNAME=Sala\n").as_deref(), Some("Sala"));
        assert_eq!(pretty_hostname("PRETTY_HOSTNAME=\"\"\n"), None);
        assert_eq!(pretty_hostname("CHASSIS=desktop\n"), None);
    }

    #[test]
    fn sizes_and_icons() {
        i18n::set(Lang::En);
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(file_icon("Foto.JPG"), "🖼");
        assert_eq!(file_icon("sin_extension"), "📄");
    }

    #[test]
    fn names_from_the_phone_are_safe() {
        assert_eq!(safe_file_name("../../.ssh/authorized_keys"), "authorized_keys");
        assert_eq!(safe_file_name("..\\..\\evil.exe"), "evil.exe");
        assert_eq!(safe_file_name(".bashrc"), "bashrc");
        assert_eq!(safe_file_name("a\u{0}b\n.txt"), "ab.txt");
        assert_eq!(safe_file_name(""), "file");
        let dir = tempfile_dir();
        std::fs::write(dir.join("foto.jpg"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "foto.jpg"), dir.join("foto (2).jpg"));
        assert_eq!(unique_path(&dir, "nueva.jpg"), dir.join("nueva.jpg"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tempfile_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("nl-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
