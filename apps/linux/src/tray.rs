// SPDX-License-Identifier: GPL-3.0-or-later
//! StatusNotifierItem, used by KDE and GNOME's AppIndicator extension.
use crate::{Msg, i18n::t};
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc;
#[derive(Default)]
pub struct TrayState {
    pub name: String,
    pub online: bool,
    pub speaker: bool,
    /// The phone's player that is playing, if any (to play it again after switching the sound).
    pub phone_playing: Option<String>,
    /// The phone's Do not disturb, when known.
    pub dnd: Option<bool>,
    pub autostart: bool,
    pub ringing: bool,
    /// When we asked the phone to ring (shared with the window: one toggle for both).
    pub ring_started: Option<std::time::Instant>,
    /// The phone this window is about.
    pub peer: Option<broche_core::DeviceId>,
    pub screen_on: bool,
}

impl TrayState {
    /// Changes when anything shown in the menu changes.
    fn fingerprint(&self) -> String {
        format!(
            "{}|{}|{}|{:?}|{:?}|{}|{}|{}",
            self.name,
            self.online,
            self.speaker,
            // Read here: the menu follows it even while the window isn't drawn.
            crate::bt_music::current().map(|p| p.on),
            self.dnd,
            self.autostart,
            self.ringing,
            self.screen_on
        )
    }
}
use zbus::zvariant::{OwnedValue, Str, Value};

/// The last activation token from the panel; used once, by the next program we open.
static ACTIVATION_TOKEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

struct Item {
    ui: mpsc::UnboundedSender<Msg>,
    ctx: eframe::egui::Context,
}
impl Item {
    fn show(&self) {
        self.ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Minimized(false));
        let _ = self.ui.send(Msg::ShowWindow);
        self.ctx.request_repaint();
    }
}
#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    fn activate(&self, _x: i32, _y: i32) {
        self.show();
    }
    fn secondary_activate(&self, _x: i32, _y: i32) {
        self.show();
    }
    fn context_menu(&self, _x: i32, _y: i32) {
        self.show();
    }
    fn scroll(&self, _delta: i32, _orientation: &str) {}
    /// GNOME's AppIndicator hands us this just before a click; whatever we launch with it
    /// (the file manager) may come to the front instead of opening behind other windows.
    fn provide_xdg_activation_token(&self, token: String) {
        *ACTIVATION_TOKEN.lock().unwrap() = Some(token);
    }
    #[zbus(property)]
    fn category(&self) -> &str {
        "Communications"
    }
    #[zbus(property)]
    fn id(&self) -> &str {
        "broche"
    }
    #[zbus(property)]
    fn title(&self) -> &str {
        "Broche"
    }
    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }
    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }
    #[zbus(property)]
    fn icon_name(&self) -> &str {
        "io.github.robertcjla.Broche"
    }
    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        let Ok(img) = image::load_from_memory(include_bytes!("../data/icon-256.png")) else { return vec![] };
        let img = img.resize(32, 32, image::imageops::FilterType::Triangle).to_rgba8();
        let bytes = img.pixels().flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
        vec![(img.width() as i32, img.height() as i32, bytes)]
    }
    #[zbus(property)]
    fn attention_icon_name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn overlay_icon_name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn menu(&self) -> zbus::zvariant::ObjectPath<'_> {
        zbus::zvariant::ObjectPath::try_from("/Menu").unwrap()
    }
}

pub async fn run(
    ui: mpsc::UnboundedSender<Msg>,
    ctx: eframe::egui::Context,
    available: Arc<AtomicBool>,
    state: Arc<RwLock<TrayState>>,
    node: Option<broche_core::Node>,
    platform: Arc<crate::platform::LinuxPlatform>,
) {
    let revision = Arc::new(std::sync::atomic::AtomicU32::new(1));
    let result = async {
        // A Flatpak may own only names under its app id (no wildcard is allowed for
        // org.kde.StatusNotifierItem-*); the watchers accept any name.
        let service = if std::env::var_os("FLATPAK_ID").is_some() {
            format!("io.github.robertcjla.Broche.StatusNotifierItem-{}-1", std::process::id())
        } else {
            format!("org.kde.StatusNotifierItem-{}-1", std::process::id())
        };
        let conn = zbus::connection::Builder::session()?
            .name(service.as_str())?
            .serve_at("/StatusNotifierItem", Item { ui: ui.clone(), ctx: ctx.clone() })?
            .serve_at(
                "/Menu",
                Menu {
                    ui: ui.clone(),
                    ctx: ctx.clone(),
                    state: state.clone(),
                    revision: revision.clone(),
                    node,
                    platform,
                },
            )?
            .build()
            .await?;
        let bus = zbus::fdo::DBusProxy::new(&conn).await?;
        let mut last = String::new();
        let mut ticks = 0u32;
        loop {
            let now = state.read().map(|s| s.fingerprint()).unwrap_or_default();
            if now != last {
                last = now;
                let rev = revision.fetch_add(1, Ordering::Relaxed) + 1;
                if let Ok(iface) = conn.object_server().interface::<_, Menu>("/Menu").await {
                    let _ = Menu::layout_updated(iface.signal_emitter(), rev, 0).await;
                }
            }
            ticks += 1;
            if ticks % 3 != 1 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
            let present = bus
                .name_has_owner(zbus::names::BusName::try_from("org.kde.StatusNotifierWatcher")?)
                .await
                .unwrap_or(false);
            if present && !available.load(Ordering::Relaxed) {
                if let Ok(watcher) = zbus::Proxy::new(
                    &conn,
                    "org.kde.StatusNotifierWatcher",
                    "/StatusNotifierWatcher",
                    "org.kde.StatusNotifierWatcher",
                )
                .await
                {
                    let ok = watcher
                        .call::<_, _, ()>("RegisterStatusNotifierItem", &(service.as_str(),))
                        .await
                        .is_ok();
                    available.store(ok, Ordering::Relaxed);
                    ctx.request_repaint();
                }
            } else if !present {
                available.store(false, Ordering::Relaxed);
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        #[allow(unreachable_code)]
        Ok::<(), zbus::Error>(())
    }
    .await;
    if let Err(e) = result {
        let _ = ui.send(Msg::Line(format!("{} {e}", t("Tray:"))));
    }
}

struct Menu {
    ui: mpsc::UnboundedSender<Msg>,
    ctx: eframe::egui::Context,
    state: Arc<RwLock<TrayState>>,
    revision: Arc<std::sync::atomic::AtomicU32>,
    node: Option<broche_core::Node>,
    platform: Arc<crate::platform::LinuxPlatform>,
}

/// Opens a folder in the file manager (xdg-open also goes through the portal in a Flatpak).
pub fn open_folder(path: &std::path::Path) {
    open(path, true);
}

/// Opens a file in the app that handles it.
pub fn open_file(path: &std::path::Path) {
    open(path, false);
}

/// `folder`: create it first if it is missing (a file must not become a folder).
fn open(path: &std::path::Path, folder: bool) {
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        if folder && let Err(e) = std::fs::create_dir_all(&path) {
            eprintln!("No se pudo preparar la carpeta {}: {e}", path.display());
            return;
        }
        let token = ACTIVATION_TOKEN.lock().unwrap().take();
        let command = |program: &str| {
            let mut c = std::process::Command::new(program);
            if let Some(token) = &token {
                c.env("XDG_ACTIVATION_TOKEN", token).env("DESKTOP_STARTUP_ID", token);
            }
            c
        };
        // Reap the child, report failures and support desktops without xdg-open.
        let opened = command("xdg-open").arg(&path).status().is_ok_and(|status| status.success());
        if !opened
            && let Err(e) = command("gio").arg("open").arg(&path).status().and_then(|status| {
                if status.success() { Ok(()) } else { Err(std::io::Error::other("gio open failed")) }
            })
        {
            eprintln!("No se pudo abrir {}: {e}", path.display());
        }
    });
}

impl Menu {
    /// Runs a menu action here, not in the window: a minimized Wayland window gets no
    /// frames, so anything queued for it would wait until it is shown again.
    fn run_here(&self, id: i32) -> bool {
        let peer = self.state.read().ok().and_then(|s| s.peer);
        match id {
            2 => open_folder(&crate::received_dir()),
            17 => {
                let saved = crate::settings::Settings::load().shared_dir;
                open_folder(&if saved.is_empty() {
                    crate::shared_dir()
                } else {
                    std::path::PathBuf::from(saved)
                });
            }
            5 => {
                let (Some(node), Some(peer)) = (self.node.clone(), peer) else { return true };
                let on = {
                    let mut st = self.state.write().unwrap();
                    let on =
                        !st.ring_started.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(30));
                    st.ring_started = on.then(std::time::Instant::now);
                    on
                };
                let ui = self.ui.clone();
                tokio::spawn(async move {
                    let line = match node.ring(peer, on).await {
                        Ok(_) => t(if on { "Ringing…" } else { "Stopped ringing." }),
                        Err(e) => format!("{} {e}", t("Failed:")),
                    };
                    let _ = ui.send(Msg::Line(line));
                });
            }
            14 => {
                let (Some(node), Some(peer)) = (self.node.clone(), peer) else { return true };
                let on = node.phone_toggles(peer).is_some_and(|tg| tg.dnd);
                tokio::spawn(async move {
                    let _ = node
                        .set_phone_toggle(peer, "dnd".into(), broche_core::PhoneToggleValue::Bool(!on))
                        .await;
                });
            }
            16 => {
                let Some(peer) = peer else { return true };
                use broche_core::Platform;
                let speaker = self.state.read().map(|s| s.speaker).unwrap_or(false);
                let _ = self.platform.set_pc_audio(&peer, Some(if speaker { 200 } else { 201 }), None);
            }
            20 => {
                let Some(on) = crate::bt_music::current().map(|p| !p.on) else { return true };
                if !crate::bt_music::claim() {
                    return true;
                }
                let (speaker, playing) = {
                    let st = self.state.read().unwrap();
                    (st.speaker, st.phone_playing.clone())
                };
                // Both at once would loop the sound between the PC and the phone.
                if on
                    && speaker
                    && let Some(peer) = peer
                {
                    use broche_core::Platform;
                    let _ =
                        self.platform.set_pc_audio(&peer, Some(broche_core::deck::PHONE_SPEAKER_OFF), None);
                }
                let resume =
                    self.node.clone().zip(peer).zip(playing).map(|((node, peer), id)| (node, peer, id));
                let ui = self.ui.clone();
                tokio::spawn(async move {
                    let line = match crate::bt_music::switch(on, resume).await {
                        Ok(line) => line,
                        Err(e) => {
                            // Asked from the tray: the window may be hidden.
                            crate::platform::desktop_notify(e.clone(), 1).await;
                            format!("{} {e}", t("Failed:"))
                        }
                    };
                    let _ = ui.send(Msg::Line(line));
                });
            }
            13 => {
                let (Some(node), Some(peer)) = (self.node.clone(), peer) else { return true };
                let ui = self.ui.clone();
                tokio::spawn(async move {
                    use ashpd::desktop::file_chooser::{FileChooserProxy, OpenFileOptions};
                    let picked = async {
                        let portal = FileChooserProxy::new().await?;
                        portal
                            .open_file(
                                None,
                                &t("Choose files to send"),
                                OpenFileOptions::default().set_multiple(true),
                            )
                            .await?
                            .response()
                    }
                    .await;
                    let Ok(sel) = picked else { return };
                    let paths: Vec<std::path::PathBuf> = sel
                        .uris()
                        .iter()
                        .filter_map(|u| url::Url::parse(u.as_str()).ok()?.to_file_path().ok())
                        .collect();
                    if paths.is_empty() {
                        return;
                    }
                    let line = match broche_core::outgoing_paths(&paths) {
                        Ok(files) => match node.send_files(peer, files).await {
                            Ok(_) => t("Transfer started."),
                            Err(e) => format!("{} {e}", t("Failed:")),
                        },
                        Err(e) => format!("{} {e}", t("Failed:")),
                    };
                    let _ = ui.send(Msg::Line(line));
                });
            }
            _ => return false,
        }
        self.ctx.request_repaint();
        true
    }
}
type Properties = HashMap<String, OwnedValue>;
/// The menu, kept short: each phone action is one entry that switches on and off.
/// 4 status · 1 open · 5 find/stop ringing · 13 send files · 7 screen on/off (always in window mode) · 19 shelf ·
/// 16 phone as speaker ✓ · 20 phone's sound here over Bluetooth ✓ · 14 Do not disturb ✓ · 40 Folders ▸ (2 received, 17 shared, 10 phone files) ·
/// 11 settings · 3 quit. 30.. are separators.
const ITEMS: &[i32] = &[4, 1, 30, 5, 13, 7, 19, 16, 20, 14, 31, 40, 11, 32, 3];
const FOLDERS: &[i32] = &[2, 17, 10];
/// Items that need the phone online.
const NEEDS_PHONE: &[i32] = &[5, 7, 10, 13, 14, 16];

fn children(id: i32) -> &'static [i32] {
    match id {
        0 => ITEMS,
        40 => FOLDERS,
        _ => &[],
    }
}

fn properties(id: i32, state: &TrayState) -> Properties {
    let mut p = HashMap::new();
    if (30..40).contains(&id) {
        p.insert("type".into(), OwnedValue::from(Str::from("separator")));
        return p;
    }
    let label = match id {
        1 => t("Open Broche"),
        2 => t("Open received files"),
        3 => t("Quit application"),
        4 => {
            if state.name.is_empty() {
                t("No paired devices.")
            } else {
                format!("{} · {}", state.name, t(if state.online { "Connected" } else { "Offline" }))
            }
        }
        5 => t(if state.ringing { "Stop ringing" } else { "Find my phone" }),
        7 => t(if state.screen_on { "Stop viewing its screen" } else { "Show its screen" }),
        10 => t("Phone files"),
        11 => t("Settings"),
        13 => t("Send files to the phone…"),
        14 => t("Do not disturb on the phone"),
        16 => t("Phone as speaker"),
        20 => t("Phone's sound on this PC (Bluetooth)"),
        17 => t("Open shared folder"),
        19 => t("Shelf"),
        40 => t("Folders"),
        _ => String::new(),
    };
    p.insert("label".into(), OwnedValue::from(Str::from(label)));
    p.insert(
        "enabled".into(),
        OwnedValue::from(
            id != 4 && (!NEEDS_PHONE.contains(&id) || state.online) && (id != 14 || state.dnd.is_some()),
        ),
    );
    let bt_music = crate::bt_music::current().map(|p| p.on);
    // Only for a phone paired over Bluetooth: most people never will.
    p.insert("visible".into(), OwnedValue::from(id != 20 || bt_music.is_some()));
    let check = match id {
        14 => Some(state.dnd.unwrap_or(false)),
        16 => Some(state.speaker),
        20 => Some(bt_music.unwrap_or(false)),
        _ => None,
    };
    if let Some(on) = check {
        p.insert("toggle-type".into(), OwnedValue::from(Str::from("checkmark")));
        p.insert("toggle-state".into(), OwnedValue::from(i32::from(on)));
    }
    if id == 0 || id == 40 {
        p.insert("children-display".into(), OwnedValue::from(Str::from("submenu")));
    }
    p
}

/// One menu entry with its children, as DBusMenu wants it.
fn layout(id: i32, state: &TrayState) -> (i32, Properties, Vec<Value<'static>>) {
    let kids = children(id).iter().map(|&c| Value::new(layout(c, state))).collect();
    (id, properties(id, state), kids)
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn get_layout(
        &self,
        parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> (u32, (i32, Properties, Vec<Value<'static>>)) {
        (self.revision.load(Ordering::Relaxed), layout(parent_id, &self.state.read().unwrap()))
    }
    fn get_group_properties(&self, ids: Vec<i32>, _property_names: Vec<String>) -> Vec<(i32, Properties)> {
        ids.into_iter().map(|id| (id, properties(id, &self.state.read().unwrap()))).collect()
    }
    fn get_property(&self, id: i32, name: &str) -> zbus::fdo::Result<OwnedValue> {
        properties(id, &self.state.read().unwrap())
            .remove(name)
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("Unknown property".into()))
    }
    fn event(&self, id: i32, event_id: &str, _data: OwnedValue, _timestamp: u32) {
        if event_id == "clicked" {
            if self.run_here(id) {
                return;
            }
            let msg = match id {
                1 => Msg::ShowWindow,
                2 => Msg::OpenReceivedFolder,
                3 => Msg::Quit,
                5..=18 => Msg::TrayAction(id),
                19 => Msg::ToggleShelf,
                _ => return,
            };
            // Wake a hidden window too: its event loop must process Quit/Open. Not for stopping
            // the phone's screen (the window minimizes itself when it was showing just the phone).
            let stopping_screen = id == 7 && self.state.read().is_ok_and(|s| s.screen_on);
            // The shelf is a window of its own: the main one stays as it is.
            if !stopping_screen && id != 19 {
                self.ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Minimized(false));
                self.ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
            }
            if matches!(id, 7 | 10) {
                // The phone's screen and files open in this window: bring it forward (works
                // with XWayland; plain Wayland doesn't let an app raise itself).
                self.ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Focus);
            }
            let _ = self.ui.send(msg);
            self.ctx.request_repaint();
        }
    }
    /// "No update needed": changes are announced with LayoutUpdated. Answering true makes
    /// GNOME's AppIndicator reload the menu, which rebuilds (and so closes) a submenu
    /// the moment it opens: the Folders submenu could never be opened.
    fn about_to_show(&self, _id: i32) -> bool {
        false
    }
    /// Tells the panel to read the menu again (labels and check marks changed).
    #[zbus(signal)]
    async fn layout_updated(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }
    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }
    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }
    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        vec![]
    }
}
