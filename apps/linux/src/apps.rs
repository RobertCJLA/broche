// SPDX-License-Identifier: GPL-3.0-or-later
//! Phone apps: the phone runs an app on a display of its own (advanced mode) and streams it,
//! and the PC shows it in a window of its own with the mouse and keyboard on it, as the
//! original NectarLink's app windows did. Each window is a deferred egui viewport: it paints
//! on its own, so it keeps going while the main window is minimized (with vsync off, see
//! `main`).
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use broche_core::{DeviceId, MirrorInput, MirrorStart, PhoneApp, TouchAction};
use eframe::egui::{self, RichText};
use tokio::sync::mpsc;

use crate::mirror::{MirrorShared, Sink};
use crate::{App, Msg, i18n::t, kit, settings, shell::fold};

/// What the Apps page remembers between frames.
#[derive(Default)]
pub struct Apps {
    pub search: String,
    /// The phone's apps, or why they couldn't be listed; `None` while loading.
    pub list: Option<(DeviceId, Result<Vec<PhoneApp>, String>)>,
    icons: HashMap<String, egui::TextureHandle>,
    pub windows: Vec<Arc<AppWindow>>,
    /// The PC's number for the next window (0 is the phone's screen).
    next_session: u32,
    /// The list was asked for while the phone was offline: ask again once it connects.
    asked_offline: bool,
}

/// Sent from a window to the task that talks to the phone.
enum WinCmd {
    Input(MirrorInput),
    Resize(u32, u32),
}

/// One app shown in a window of its own.
pub struct AppWindow {
    pub peer: DeviceId,
    pub session: u32,
    pub title: String,
    shared: Arc<MirrorShared>,
    cmd: mpsc::UnboundedSender<WinCmd>,
    tex: Mutex<Option<egui::TextureHandle>>,
    touching: AtomicBool,
    /// Pictures have arrived: the stream is up.
    seen: AtomicBool,
    /// Closed by the user, or the phone stopped it; the main window then tidies up.
    pub closed: AtomicBool,
    /// The window's size in pixels as last told to the phone, and a size waiting to settle.
    sent: Mutex<(u32, u32)>,
    pending: Mutex<((u32, u32), Instant)>,
}

impl AppWindow {
    fn viewport(&self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of(("phone-app", self.session))
    }

    fn close(&self, ctx: &egui::Context) {
        self.closed.store(true, Ordering::Release);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    /// Draws the window: called by egui for this viewport, from any frame of its own.
    fn show(&self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) {
            self.close(ctx);
            return;
        }
        let active = self.shared.active.load(Ordering::Acquire);
        if active {
            self.seen.store(true, Ordering::Release);
        } else if self.seen.load(Ordering::Acquire) {
            // The phone stopped it (the app closed, the phone went away).
            self.close(ctx);
            return;
        }
        if let Some(img) = self.shared.frame.lock().unwrap().take() {
            let mut tex = self.tex.lock().unwrap();
            match tex.as_mut() {
                Some(t) => t.set(img, Default::default()),
                None => {
                    *tex = Some(ctx.load_texture(format!("app-{}", self.session), img, Default::default()))
                }
            }
        }
        if active {
            self.follow_size(ctx);
            self.keys(ctx);
        }
        let tex = self.tex.lock().unwrap().clone();
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(egui::Color32::BLACK)).show(ctx, |ui| {
            match &tex {
                Some(tex) => {
                    let mut touching = self.touching.load(Ordering::Relaxed);
                    touch_surface(ui, tex, &mut touching, |input| {
                        let _ = self.cmd.send(WinCmd::Input(input));
                    });
                    self.touching.store(touching, Ordering::Relaxed);
                }
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new(t("Opening the app on the phone…")).color(egui::Color32::WHITE),
                            );
                        });
                    });
                }
            }
        });
    }

    /// The phone gives the app a display the size of the window, once the size settles.
    fn follow_size(&self, ctx: &egui::Context) {
        let px = ctx.content_rect().size() * ctx.pixels_per_point();
        let now = (px.x.round() as u32, px.y.round() as u32);
        let mut pending = self.pending.lock().unwrap();
        if pending.0 != now {
            *pending = (now, Instant::now());
            ctx.request_repaint_after(SETTLE);
            return;
        }
        let mut sent = self.sent.lock().unwrap();
        let waited = pending.1.elapsed();
        if *sent == now {
            return;
        }
        if waited >= SETTLE {
            *sent = now;
            let _ = self.cmd.send(WinCmd::Resize(now.0, now.1));
        } else {
            // A picture from the phone drawn in the meantime used up the earlier request: ask
            // again, or a still app (nothing more to draw) never got its new size.
            ctx.request_repaint_after(SETTLE - waited);
        }
    }

    /// Typing goes to the app; Escape is the phone's Back.
    fn keys(&self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let input = match event {
                egui::Event::Text(text) => MirrorInput::Text { text },
                egui::Event::Paste(text) => MirrorInput::Text { text },
                egui::Event::Key { key, pressed: true, modifiers, .. }
                    if !modifiers.ctrl && !modifiers.alt =>
                {
                    let Some(name) = phone_key(key) else { continue };
                    MirrorInput::Key { key: name.into() }
                }
                _ => continue,
            };
            let _ = self.cmd.send(WinCmd::Input(input));
        }
    }
}

/// How long the window keeps a size before the phone hears it (dragging an edge resizes
/// many times a second; each change remakes the phone's display).
const SETTLE: Duration = Duration::from_millis(400);

/// The phone's name for a key (`Elevated.keys` on Android), for keys that don't type text.
fn phone_key(key: egui::Key) -> Option<&'static str> {
    use egui::Key;
    Some(match key {
        Key::Escape => "back",
        Key::Enter => "enter",
        Key::Backspace => "backspace",
        Key::Delete => "delete",
        Key::ArrowLeft => "left",
        Key::ArrowRight => "right",
        Key::ArrowUp => "up",
        Key::ArrowDown => "down",
        Key::Tab => "tab",
        _ => return None,
    })
}

/// A phone's picture, as large as fits, taking clicks, drags and scrolling as touches.
/// Shared by the phone's screen and the app windows.
pub fn touch_surface(
    ui: &mut egui::Ui,
    tex: &egui::TextureHandle,
    touching: &mut bool,
    send: impl Fn(MirrorInput),
) {
    let avail = ui.available_size();
    let size = tex.size_vec2();
    let scale = (avail.x / size.x).min(avail.y / size.y).max(0.05);
    let (rect, _) = ui
        .with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.allocate_exact_size(size * scale, egui::Sense::click_and_drag())
        })
        .inner;
    ui.painter().image(
        tex.id(),
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
    let frac = |p: egui::Pos2| {
        (
            ((p.x - rect.min.x) / rect.width()).clamp(0.0, 1.0),
            ((p.y - rect.min.y) / rect.height()).clamp(0.0, 1.0),
        )
    };
    let (pressed, down, released, pos, scroll) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.pointer.interact_pos(),
            i.smooth_scroll_delta,
        )
    });
    let Some(p) = pos else { return };
    let inside = rect.contains(p);
    let (x, y) = frac(p);
    if pressed && inside {
        send(MirrorInput::Touch { action: TouchAction::Down, x, y });
        // A quick click can press and release within one frame: lift the finger too.
        *touching = !released;
        if released {
            send(MirrorInput::Touch { action: TouchAction::Up, x, y });
        }
    } else if *touching && released {
        *touching = false;
        send(MirrorInput::Touch { action: TouchAction::Up, x, y });
    } else if *touching && down {
        send(MirrorInput::Touch { action: TouchAction::Move, x, y });
    }
    if inside && scroll != egui::Vec2::ZERO {
        send(MirrorInput::Scroll { x, y, dx: scroll.x / 40.0, dy: -scroll.y / 40.0 });
    }
}

impl App {
    /// Asks the phone for its apps (on opening the page, or Refresh).
    pub(crate) fn load_apps(&mut self) {
        let Some(peer) = self.peer() else { return };
        self.apps.list = None;
        self.apps.asked_offline = !self.online(peer);
        self.load(move |n| async move {
            Ok(Msg::PhoneApps(peer, n.mirror_apps(peer).await.map_err(|e| e.to_string())))
        });
    }

    fn online(&self, peer: DeviceId) -> bool {
        self.devices.iter().any(|d| d.id == peer.to_string() && d.state.starts_with("online"))
    }

    pub(crate) fn got_apps(
        &mut self,
        ctx: &egui::Context,
        peer: DeviceId,
        apps: Result<Vec<PhoneApp>, String>,
    ) {
        if let Ok(list) = &apps {
            for app in list {
                if self.apps.icons.contains_key(&app.pkg) {
                    continue;
                }
                if let Some(img) = app.icon.as_deref().and_then(|b| image::load_from_memory(b).ok()) {
                    let rgba = img.to_rgba8();
                    let ci = egui::ColorImage::from_rgba_unmultiplied(
                        [rgba.width() as usize, rgba.height() as usize],
                        rgba.as_raw(),
                    );
                    let tex = ctx.load_texture(format!("app-icon-{}", app.pkg), ci, Default::default());
                    self.apps.icons.insert(app.pkg.clone(), tex);
                }
            }
        }
        self.apps.list = Some((peer, apps));
    }

    pub(crate) fn apps_tab(&mut self, ui: &mut egui::Ui) {
        let Some(peer) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        let failed_offline = self.apps.asked_offline && matches!(self.apps.list, Some((_, Err(_))));
        if self.apps.list.as_ref().is_some_and(|(p, _)| *p != peer) || (failed_offline && self.online(peer)) {
            self.load_apps();
        }
        kit::supporting(
            ui,
            t("Open a phone app in a window of its own on this PC. It needs advanced mode on the phone."),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let w = ui.available_width() - kit::button_width(ui, RichText::new(t("Refresh")), 0.0) - 8.0;
            ui.allocate_ui(egui::vec2(w.max(120.0), 40.0), |ui| {
                kit::search_field(ui, &mut self.apps.search, &t("Search apps"));
            });
            if kit::tonal_button(ui, &t("Refresh")).clicked() {
                self.load_apps();
            }
        });
        ui.add_space(8.0);

        let open: Vec<(u32, String)> = self
            .apps
            .windows
            .iter()
            .filter(|w| w.peer == peer)
            .map(|w| (w.session, w.title.clone()))
            .collect();
        if !open.is_empty() {
            kit::card(ui, |ui| {
                kit::title(ui, &t("Open windows"));
                for (session, title) in open {
                    ui.horizontal(|ui| {
                        ui.label(&title);
                        if kit::tonal_button(ui, &t("Close")).clicked()
                            && let Some(w) = self.apps.windows.iter().find(|w| w.session == session)
                        {
                            w.close(ui.ctx());
                        }
                    });
                }
            });
            ui.add_space(8.0);
        }

        let apps = match &self.apps.list {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t("Loading the phone's apps…"));
                });
                return;
            }
            Some((_, Err(e))) => {
                kit::supporting(
                    ui,
                    format!("{} {}", t("Can't list the phone's apps:"), crate::i18n::core_errors(e)),
                );
                return;
            }
            Some((_, Ok(apps))) => apps.clone(),
        };
        let query = fold(self.apps.search.trim());
        let shown: Vec<&PhoneApp> =
            apps.iter().filter(|a| query.is_empty() || fold(&a.label).contains(&query)).collect();
        if shown.is_empty() {
            kit::supporting(ui, t("No apps match."));
            return;
        }
        let mut chosen = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // As many columns as fit, stretched to the full width (fixed widths left a gap).
            let gap = ui.spacing().item_spacing.x;
            let (_, width) = kit::columns_for(ui.available_width() - 1.0, 200.0, gap, 8);
            ui.horizontal_wrapped(|ui| {
                for app in shown {
                    let size = egui::vec2(32.0, 32.0);
                    let button = match self.apps.icons.get(&app.pkg) {
                        Some(tex) => egui::Button::image_and_text(
                            egui::Image::new(tex).fit_to_exact_size(size),
                            &app.label,
                        ),
                        None => egui::Button::new(&app.label),
                    };
                    let r = ui.add_sized([width, 48.0], button.truncate());
                    if r.clicked() {
                        chosen = Some(app.clone());
                    }
                }
            });
        });
        if let Some(app) = chosen {
            self.open_app(ui.ctx(), peer, app);
        }
    }

    /// Opens `app` from the phone in a window of its own.
    pub(crate) fn open_app(&mut self, ctx: &egui::Context, peer: DeviceId, app: PhoneApp) {
        self.apps.next_session += 1;
        let session = self.apps.next_session;
        let shared = Arc::new(MirrorShared::default());
        let (tx, mut rx) = mpsc::unbounded_channel();
        let phone = self
            .devices
            .iter()
            .find(|d| d.id == peer.to_string())
            .map(|d| d.name.clone())
            .unwrap_or_default();
        let window = Arc::new(AppWindow {
            peer,
            session,
            title: if phone.is_empty() { app.label.clone() } else { format!("{} · {phone}", app.label) },
            shared: shared.clone(),
            cmd: tx,
            tex: Mutex::new(None),
            touching: AtomicBool::new(false),
            seen: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            sent: Mutex::new((0, 0)),
            pending: Mutex::new(((0, 0), Instant::now())),
        });
        *shared.ctx.lock().unwrap() = Some(ctx.clone());
        *shared.viewport.lock().unwrap() = Some(window.viewport());
        // Ready before the phone's first packet: the core asks for the sink when it arrives.
        self.platform.app_sinks.lock().unwrap().insert(session, Sink::new("app", shared));
        self.apps.windows.push(window.clone());
        let video = settings::Settings::load();
        let fps = video.video_fps();
        let ctx = ctx.clone();
        self.act(&format!("open app {}", app.pkg), move |n| async move {
            let options = MirrorStart {
                max_size: video.video_max_size(),
                fps,
                bitrate: video.video_bitrate(),
                audio: false,
                session,
                app: Some(app.pkg.clone()),
                stay_awake: false,
                screen_off: false,
            };
            if let Err(e) = n.mirror_start(peer, options).await {
                window.close(&ctx);
                return Err(e.to_string());
            }
            tokio::spawn(async move {
                while let Some(cmd) = rx.recv().await {
                    let sent = match cmd {
                        WinCmd::Input(input) => n.mirror_input(peer, session, input).await,
                        WinCmd::Resize(w, h) => n.mirror_resize(peer, session, w, h).await,
                    };
                    if sent.is_err() {
                        break;
                    }
                }
            });
            Ok(t("The app opens in a window of its own."))
        });
    }

    /// Every frame of the main window: keeps the app windows open, and tidies up closed ones.
    pub(crate) fn app_windows(&mut self, ctx: &egui::Context) {
        let (closed, open): (Vec<_>, Vec<_>) = std::mem::take(&mut self.apps.windows)
            .into_iter()
            .partition(|w| w.closed.load(Ordering::Acquire));
        self.apps.windows = open;
        for w in closed {
            self.platform.app_sinks.lock().unwrap().remove(&w.session);
            let (peer, session) = (w.peer, w.session);
            if let Some(node) = self.node.clone() {
                self.rt.spawn(async move { node.mirror_stop(peer, session).await });
            }
        }
        for w in &self.apps.windows {
            let w2 = w.clone();
            ctx.show_viewport_deferred(
                w.viewport(),
                egui::ViewportBuilder::default()
                    .with_title(&w.title)
                    .with_app_id("io.github.robertcjla.Broche")
                    .with_inner_size([380.0, 800.0])
                    .with_min_inner_size([240.0, 240.0]),
                move |ctx, _| w2.show(ctx),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A click whose press and release land in one frame is a whole tap, not a held finger.
    #[test]
    fn quick_click_in_one_frame_lifts_the_finger() {
        let ctx = egui::Context::default();
        let tex = ctx.load_texture(
            "t",
            egui::ColorImage::filled([10, 20], egui::Color32::BLACK),
            Default::default(),
        );
        let pos = egui::pos2(200.0, 200.0);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 800.0))),
            events: vec![egui::Event::PointerMoved(pos), button(true), button(false)],
            ..Default::default()
        };
        let sent = std::cell::RefCell::new(vec![]);
        let mut touching = false;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                touch_surface(ui, &tex, &mut touching, |i| {
                    if let MirrorInput::Touch { action, .. } = i {
                        sent.borrow_mut().push(action);
                    }
                });
            });
        });
        assert_eq!(*sent.borrow(), vec![TouchAction::Down, TouchAction::Up]);
        assert!(!touching);
    }
}
