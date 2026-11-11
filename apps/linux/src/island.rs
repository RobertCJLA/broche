// SPDX-License-Identifier: GPL-3.0-or-later
//! The island: a pill at the top centre of the screen, above other windows, while there is a
//! call, a transfer or music, as the original NectarLink's Island. It grows when the pointer is
//! on it or a call rings, and hides when nothing is going on. A deferred egui viewport (it keeps
//! going while the main window is minimized), cut to a pill with X11's shape extension.
use crate::settings::IslandPlace;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use broche_core::{
    CallCommand, CallState, DeviceId, MediaAction, MediaPlayer, Node, Transfer, TransferState,
};
use eframe::egui::{self, Color32, RichText};
use tokio::runtime::Handle;

use crate::{App, i18n::t};

/// The window's title: how the shape thread finds it (not shown: no title bar, not in lists).
const TITLE: &str = "Broche island";
const SMALL: egui::Vec2 = egui::vec2(340.0, 40.0);
const LARGE: egui::Vec2 = egui::vec2(400.0, 96.0);
/// Below GNOME's top bar.
const TOP: f32 = 40.0;
const FILL: Color32 = Color32::from_rgb(18, 18, 22);

/// What the island shows, most important first.
#[derive(Clone)]
pub enum Activity {
    Call {
        peer: DeviceId,
        call: CallState,
    },
    Transfer {
        name: String,
        more: usize,
        done: u64,
        total: u64,
        direction: Option<broche_core::Direction>,
        state: TransferState,
    },
    /// `phone` is `None` for this PC's own player.
    Music {
        phone: Option<DeviceId>,
        player: MediaPlayer,
        art: Option<egui::TextureHandle>,
    },
}

#[derive(Default)]
pub(crate) struct Transfers {
    items: Vec<(String, Transfer)>,
    finished_at: Option<std::time::Instant>,
}

impl Transfers {
    pub(crate) fn contains(&self, transfer: &Transfer) -> bool {
        let key = format!("native:{}:{:?}:{}", transfer.device, transfer.direction, transfer.id);
        self.items.iter().any(|(id, _)| *id == key)
    }

    pub(crate) fn update(&mut self, transfer: &Transfer) {
        self.update_key(
            format!("native:{}:{:?}:{}", transfer.device, transfer.direction, transfer.id),
            transfer.clone(),
        );
    }

    pub(crate) fn download(&mut self, generation: u64, id: usize, item: &crate::Download) {
        let state = match &item.result {
            Some(Ok(())) => TransferState::Done { saved: vec![] },
            Some(Err(error)) => TransferState::Failed(broche_core::TransferFailure::Other(error.clone())),
            None if item.started => TransferState::Running,
            None => TransferState::Waiting,
        };
        self.update_key(
            format!("download:{generation}:{id}"),
            Transfer {
                id: String::new(),
                device: item.device,
                direction: broche_core::Direction::Incoming,
                names: vec![item.name.clone()],
                files: 1,
                total: item.total,
                done: item.done,
                state,
                open_on_arrival: false,
            },
        );
    }

    fn update_key(&mut self, key: String, transfer: Transfer) {
        if self.finished_at.is_some() && !transfer.state.is_finished() {
            self.items.clear();
            self.finished_at = None;
        }
        if let Some((_, old)) = self.items.iter_mut().find(|(id, _)| *id == key) {
            *old = transfer;
        } else {
            self.items.push((key, transfer));
        }
        if self.items.iter().all(|(_, item)| item.state.is_finished()) {
            self.finished_at.get_or_insert_with(std::time::Instant::now);
        } else {
            self.finished_at = None;
        }
    }

    pub(crate) fn activity(&self, linger: u32) -> Option<Activity> {
        if self.finished_at.is_some_and(|at| at.elapsed().as_secs() >= u64::from(linger)) {
            return None;
        }
        let first = &self.items.first()?.1;
        let (done, total, files) =
            self.items.iter().fold((0u64, 0u64, 0usize), |(done, total, files), (_, item)| {
                (
                    done.saturating_add(item.done.min(item.total)),
                    total.saturating_add(item.total),
                    files.saturating_add(item.files),
                )
            });
        let state = if self.finished_at.is_some() {
            self.items
                .iter()
                .find_map(|(_, item)| match &item.state {
                    TransferState::Failed(_) => Some(item.state.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| {
                    if self.items.iter().any(|(_, item)| item.state == TransferState::Cancelled) {
                        TransferState::Cancelled
                    } else {
                        TransferState::Done { saved: vec![] }
                    }
                })
        } else if self.items.iter().any(|(_, item)| item.state == TransferState::Running) {
            TransferState::Running
        } else if self.items.iter().any(|(_, item)| item.state == TransferState::Paused) {
            TransferState::Paused
        } else {
            TransferState::Waiting
        };
        Some(Activity::Transfer {
            name: first.names.first().cloned().unwrap_or_default(),
            more: files.saturating_sub(1),
            done,
            total,
            direction: self
                .items
                .iter()
                .all(|(_, item)| item.direction == first.direction)
                .then_some(first.direction),
            state,
        })
    }
}

impl Activity {
    /// A ringing call opens the island wide at once.
    fn urgent(&self) -> bool {
        matches!(self, Activity::Call { call, .. } if call.incoming && call.state == "ringing")
    }
}

/// The island while it is on.
pub struct Island {
    node: Node,
    rt: Handle,
    pub activity: Mutex<Option<Activity>>,
    large: AtomicBool,
    shown: AtomicBool,
    /// The pointer is on the island, as X11 says (egui's own idea can be stale: the window
    /// gets a made-up pointer position when it appears, and no "left" after it).
    hovered: Arc<AtomicBool>,
    shape: mpsc::Sender<(u16, u16)>,
    shaped: Mutex<(u16, u16)>,
    /// This PC's album art, by key.
    pc_art: Mutex<HashMap<String, egui::TextureHandle>>,
    /// What a button couldn't do, shown on the island for a moment (and in the status bar).
    failed: Failed,
    /// Where it goes: a place on the main screen, or where it was dragged.
    place: Mutex<IslandPlace>,
    /// Dragging is off (Preferences › Window).
    pub locked: AtomicBool,
    /// The anchor changed from Preferences: move the window on its next frame.
    moved: AtomicBool,
}

/// The last failure of a button, shared with the task that ran it.
#[derive(Clone)]
struct Failed {
    last: Arc<Mutex<Option<(String, std::time::Instant)>>>,
    ui: tokio::sync::mpsc::UnboundedSender<crate::Msg>,
    ctx: egui::Context,
}

impl Failed {
    fn report(&self, result: Result<(), String>) {
        let Err(e) = result else { return };
        let text = crate::i18n::core_errors(&e);
        let _ = self.ui.send(crate::Msg::Line(format!("{} {text}", t("Failed:"))));
        *self.last.lock().unwrap() = Some((text, std::time::Instant::now()));
        self.ctx.request_repaint_of(Island::viewport());
    }

    /// The failure to show now, for a few seconds after it happened.
    fn recent(&self) -> Option<String> {
        let last = self.last.lock().unwrap();
        last.as_ref().filter(|(_, at)| at.elapsed().as_secs() < 5).map(|(e, _)| e.clone())
    }
}

impl Island {
    pub fn new(
        node: Node,
        rt: Handle,
        ctx: &egui::Context,
        ui: tokio::sync::mpsc::UnboundedSender<crate::Msg>,
    ) -> Arc<Self> {
        let failed = Failed { last: Arc::default(), ui, ctx: ctx.clone() };
        let saved = crate::settings::Settings::load();
        let (tx, rx) = mpsc::channel();
        let hovered = Arc::new(AtomicBool::new(false));
        let (h, ctx) = (hovered.clone(), ctx.clone());
        std::thread::spawn(move || {
            if let Err(e) = shape_thread(&rx, &h, &ctx) {
                eprintln!("island shape unavailable: {e}");
                while rx.recv().is_ok() {}
            }
        });
        Arc::new(Self {
            node,
            rt,
            activity: Mutex::new(None),
            large: AtomicBool::new(false),
            shown: AtomicBool::new(false),
            hovered,
            shape: tx,
            shaped: Mutex::new((0, 0)),
            pc_art: Mutex::new(HashMap::new()),
            failed,
            place: Mutex::new(saved.island_at),
            locked: AtomicBool::new(saved.island_locked),
            moved: AtomicBool::new(false),
        })
    }

    /// The window's top-left corner for `size` (points), worked out from the main screen's
    /// real size each time, so a corner is a corner on any screen and at either island size.
    fn corner(&self, ppp: f32, size: egui::Vec2) -> egui::Pos2 {
        const MARGIN: f32 = 16.0;
        let (area, whole) = screen_area().unwrap_or((
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0)),
            egui::vec2(1920.0, 1080.0),
        ));
        let (area, whole) = (area / ppp, whole / ppp);
        let middle = area.center().x - size.x / 2.0;
        let (x, y) = match *self.place.lock().unwrap() {
            IslandPlace::TopCentre => (middle, area.top() + TOP),
            IslandPlace::TopLeft => (area.left() + MARGIN, area.top() + TOP),
            IslandPlace::TopRight => (area.right() - size.x - MARGIN, area.top() + TOP),
            IslandPlace::Bottom => (middle, area.bottom() - size.y - MARGIN),
            IslandPlace::At(x, y) => (x as f32 - size.x / 2.0, y as f32),
        };
        egui::pos2(x.clamp(0.0, (whole.x - size.x).max(0.0)), y.clamp(0.0, (whole.y - size.y).max(0.0)))
    }

    /// Puts the island at `place` now (Preferences).
    pub fn set_place(&self, ctx: &egui::Context, place: IslandPlace) {
        *self.place.lock().unwrap() = place;
        self.moved.store(true, Ordering::Release);
        ctx.request_repaint_of(Self::viewport());
    }

    /// Dragging the island by its background moves it; where it is let go is remembered.
    fn drag(&self, ui: &egui::Ui, size: egui::Vec2) {
        if self.locked.load(Ordering::Acquire) {
            return;
        }
        let ctx = ui.ctx();
        let id = egui::Id::new("island-move");
        let r = ui.interact(ui.max_rect(), id, egui::Sense::drag());
        // The pointer on the screen, from X11: egui's is inside the window, which moves under
        // it, so following that lagged behind and the island jumped away from the pointer.
        let ppp = ctx.pixels_per_point();
        let pointer = || screen_pointer().map(|(x, y)| egui::pos2(x, y));
        if r.drag_started()
            && let Some(origin) = ctx.input(|i| i.pointer.press_origin())
        {
            ctx.data_mut(|d| d.insert_temp(id, origin.to_vec2() * ppp));
        }
        if r.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            if let (Some(grab), Some(p)) = (ctx.data(|d| d.get_temp::<egui::Vec2>(id)), pointer()) {
                let corner = dragged_corner(p, grab, ppp);
                *self.place.lock().unwrap() =
                    IslandPlace::At((corner.x + size.x / 2.0).round() as i32, corner.y.round() as i32);
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(self.corner(ppp, size)));
            }
            ctx.request_repaint();
        } else if r.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
        if r.drag_stopped() {
            let place = *self.place.lock().unwrap();
            crate::settings::Settings::update(|s| s.island_at = place);
        }
    }

    fn viewport() -> egui::ViewportId {
        egui::ViewportId::from_hash_of("island")
    }

    /// Draws the island: called by egui for its viewport.
    fn show(&self, ctx: &egui::Context) {
        let Some(activity) = self.activity.lock().unwrap().clone() else { return };
        let large = expanded(
            self.hovered.load(Ordering::Acquire),
            activity.urgent(),
            ctx.input(|i| i.pointer.primary_down()),
            self.large.load(Ordering::Acquire),
        );
        let size = if large { LARGE } else { SMALL };
        if self.large.swap(large, Ordering::AcqRel) != large || self.moved.swap(false, Ordering::AcqRel) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                self.corner(ctx.pixels_per_point(), size),
            ));
        }
        let px = (ctx.content_rect().size() * ctx.pixels_per_point()).round();
        let px = (px.x as u16, px.y as u16);
        let mut shaped = self.shaped.lock().unwrap();
        if *shaped != px {
            *shaped = px;
            let _ = self.shape.send(px);
        }
        drop(shaped);

        let frame = egui::Frame::NONE.fill(FILL).inner_margin(egui::Margin::symmetric(16, 6));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            // First, so the buttons drawn after it get their clicks.
            self.drag(ui, size);
            ui.visuals_mut().override_text_color = Some(Color32::WHITE);
            if let Some(error) = self.failed.recent() {
                ui.horizontal_centered(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("⚠ {error}")).color(Color32::from_rgb(255, 140, 140)),
                        )
                        .truncate(),
                    )
                    .on_hover_text(error);
                });
                return;
            }
            match &activity {
                Activity::Call { peer, call } => self.call(ui, *peer, call, large),
                Activity::Transfer { .. } => transfer(ui, &activity, large),
                Activity::Music { phone, player, art } => {
                    let art = art.clone().or_else(|| self.pc_texture(ctx, player));
                    self.music(ui, *phone, player, art.as_ref(), large)
                }
            }
        });
        // The call's clock and a transfer's progress move on without an event every second.
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }

    fn call(&self, ui: &mut egui::Ui, peer: DeviceId, call: &CallState, large: bool) {
        if let Some(cmd) = call_content(ui, call, large) {
            let (node, call, failed) = (self.node.clone(), call.clone(), self.failed.clone());
            self.rt.spawn(async move {
                failed.report(crate::bt_calls::dispatch(&node, peer, &call, cmd).await);
            });
        }
        if large
            && call.state == "active"
            && let Some(ag) = crate::bt_calls::audio_gateway(call.number.as_deref().unwrap_or_default())
        {
            let on_pc = ag.transport_state == "active";
            if crate::kit::named(
                ui.add_enabled(
                    !on_pc,
                    egui::Button::new(if on_pc { t("Audio on this PC") } else { t("Talk on this PC") }),
                ),
                &t("Talk on this PC"),
            )
            .clicked()
            {
                let failed = self.failed.clone();
                let node = self.node.clone();
                self.rt.spawn(async move {
                    failed.report(crate::bt_calls::activate_audio(&node, peer, &ag.path).await);
                });
            }
        }
    }

    fn music(
        &self,
        ui: &mut egui::Ui,
        phone: Option<DeviceId>,
        player: &MediaPlayer,
        art: Option<&egui::TextureHandle>,
        large: bool,
    ) {
        let title = player.title.clone().unwrap_or_default();
        let artist = player.artist.clone().unwrap_or_default();
        ui.horizontal_centered(|ui| {
            let side = if large { 64.0 } else { 26.0 };
            match art {
                Some(tex) => {
                    ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(side, side)).corner_radius(6));
                }
                None => {
                    ui.label(RichText::new("♪").size(18.0));
                }
            }
            if large {
                let mut action = None;
                centered_details(ui, 36.0, |ui| {
                    ui.add(egui::Label::new(RichText::new(&title).strong()).truncate());
                    ui.add(egui::Label::new(RichText::new(&artist).small()).truncate());
                    ui.horizontal(|ui| {
                        let toggle = if player.playing {
                            ("⏸", MediaAction::Pause)
                        } else {
                            ("▶", MediaAction::Play)
                        };
                        // Targets of 36 px (they were the size of the text); play/pause stands
                        // out in a light circle, like the phone's music widget.
                        for (icon, a) in [("⏮", MediaAction::Previous), toggle, ("⏭", MediaAction::Next)]
                        {
                            let main = a == toggle.1;
                            let text = RichText::new(icon).size(18.0);
                            let button = if main {
                                egui::Button::new(text.color(FILL))
                                    .fill(Color32::from_gray(235))
                                    .stroke(egui::Stroke::NONE)
                                    .corner_radius(18)
                            } else {
                                egui::Button::new(text.color(Color32::from_gray(235))).frame(false)
                            };
                            if ui.add(button.min_size(egui::vec2(36.0, 36.0))).clicked() {
                                action = Some(a);
                            }
                        }
                    });
                });
                if let Some(action) = action {
                    let (node, id, failed) = (self.node.clone(), player.id.clone(), self.failed.clone());
                    self.rt.spawn(async move {
                        let result = match phone {
                            Some(peer) => {
                                node.media_command(peer, id, action, None).await.map_err(|e| e.to_string())
                            }
                            None => crate::pc_media::command(&id, action, None).await,
                        };
                        failed.report(result);
                    });
                }
            } else {
                let line = if artist.is_empty() { title } else { format!("{title} · {artist}") };
                ui.add(egui::Label::new(line).truncate());
            }
        });
    }

    /// This PC's album art as a texture (the phone's comes from the main window).
    fn pc_texture(&self, ctx: &egui::Context, player: &MediaPlayer) -> Option<egui::TextureHandle> {
        let key = player.art_key.clone()?;
        let mut cache = self.pc_art.lock().unwrap();
        if let Some(tex) = cache.get(&key) {
            return Some(tex.clone());
        }
        let img = image::load_from_memory(player.art.as_deref()?).ok()?.to_rgba8();
        let ci = egui::ColorImage::from_rgba_unmultiplied(
            [img.width() as usize, img.height() as usize],
            img.as_raw(),
        );
        let tex = ctx.load_texture(format!("island-art-{key}"), ci, Default::default());
        if cache.len() > 16 {
            cache.clear();
        }
        cache.insert(key, tex.clone());
        Some(tex)
    }
}

fn call_content(ui: &mut egui::Ui, call: &CallState, large: bool) -> Option<CallCommand> {
    let mut cmd = None;
    let who = call.name.clone().or(call.number.clone()).unwrap_or_else(|| t("Unknown caller"));
    let ringing = call.incoming && call.state == "ringing";
    let status = if ringing {
        t("Incoming call")
    } else if let Some(since) = call.since {
        let secs = (now_ms() - since).max(0) / 1000;
        format!("{} {}:{:02}", t("On a call"), secs / 60, secs % 60)
    } else {
        t("Calling…")
    };
    let height = if large { 28.0 } else { ui.available_height() };
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            compact_row(ui, "📞", &who, &status);
        },
    );
    if large {
        ui.horizontal_wrapped(|ui| {
            if ui.add(red_button(if ringing { t("Decline") } else { t("Hang up") })).clicked() {
                cmd = Some(CallCommand::Decline);
            }
            if ringing {
                if ui.add(green_button(t("Answer"))).clicked() {
                    cmd = Some(CallCommand::Answer);
                }
            } else if let Some(controls) = &call.controls {
                if ui.selectable_label(controls.speaker, t("Speaker")).clicked() {
                    cmd = Some(CallCommand::Speaker(!controls.speaker));
                }
                if ui.selectable_label(controls.muted, t("Mute")).clicked() {
                    cmd = Some(CallCommand::Mute(!controls.muted));
                }
            }
        });
    }
    cmd
}

fn compact_row(ui: &mut egui::Ui, icon: &str, title: &str, trailing: &str) {
    ui.horizontal_centered(|ui| {
        ui.label(RichText::new(icon).size(18.0));
        let tail = ui.painter().layout_no_wrap(
            trailing.to_owned(),
            egui::TextStyle::Body.resolve(ui.style()),
            Color32::WHITE,
        );
        let width = (ui.available_width() - tail.size().x - ui.spacing().item_spacing.x).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(width, ui.available_height()),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(width);
                ui.add(egui::Label::new(RichText::new(title).strong()).truncate());
            },
        );
        ui.label(trailing);
    });
}

fn centered_details(ui: &mut egui::Ui, controls_height: f32, contents: impl FnOnce(&mut egui::Ui)) {
    let height = ui.text_style_height(&egui::TextStyle::Body)
        + ui.text_style_height(&egui::TextStyle::Small)
        + 2.0 * ui.spacing().item_spacing.y
        + controls_height;
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), height),
        egui::Layout::top_down(egui::Align::Min),
        contents,
    );
}

fn transfer(ui: &mut egui::Ui, activity: &Activity, large: bool) {
    let Activity::Transfer { name, more, done, total, direction, state } = activity else { return };
    let share = if matches!(state, TransferState::Done { .. }) {
        1.0
    } else if *total > 0 {
        (*done as f64 / *total as f64).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let name = if *more > 0 { format!("{name} (+{more})") } else { name.to_owned() };
    let (icon, moving) = match direction {
        Some(broche_core::Direction::Incoming) => ("⬇", t("Receiving")),
        Some(broche_core::Direction::Outgoing) => ("⬆", t("Sending")),
        None => ("↕", t("Transferring files")),
    };
    let status = match state {
        TransferState::Waiting | TransferState::Requested => t("Waiting for the other device"),
        TransferState::Paused => t("Paused"),
        TransferState::Failed(_) => t("Failed:"),
        TransferState::Cancelled => t("Cancelled"),
        TransferState::Done { .. } => t("Transfer complete"),
        TransferState::Running => moving,
    };
    if !large {
        let trailing = if matches!(state, TransferState::Running | TransferState::Done { .. }) {
            format!("{:.0} %", share * 100.0)
        } else {
            status
        };
        compact_row(ui, icon, &name, &trailing);
        return;
    }
    ui.horizontal_centered(|ui| {
        ui.label(RichText::new(icon).size(18.0));
        centered_details(ui, 6.0, |ui| {
            ui.add(egui::Label::new(RichText::new(&name).strong()).truncate());
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "{} · {} / {}",
                        status,
                        crate::human_size(*done),
                        crate::human_size(*total)
                    ))
                    .small(),
                )
                .truncate(),
            );
            ui.add(egui::ProgressBar::new(share).desired_height(6.0));
        });
    });
}

fn red_button(text: String) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(Color32::from_rgb(200, 50, 60))
}

fn green_button(text: String) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(Color32::from_rgb(40, 150, 80))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn dragged_corner(pointer: egui::Pos2, grab: egui::Vec2, scale: f32) -> egui::Pos2 {
    (pointer - grab) / scale
}

fn expanded(hovered: bool, urgent: bool, pressed: bool, current: bool) -> bool {
    if pressed { current } else { hovered || urgent }
}

fn viewport_builder(
    corner: egui::Pos2,
    size: egui::Vec2,
    urgent: bool,
    initial: bool,
) -> egui::ViewportBuilder {
    let builder = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_app_id("io.github.robertcjla.Broche")
        .with_decorations(false)
        .with_window_level(if urgent { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal })
        .with_taskbar(false)
        .with_active(false)
        // Mutter lowers normal docks on a fullscreen monitor; only ringing calls stay above.
        .with_window_type(egui::X11WindowType::Dock)
        .with_resizable(false)
        .with_inner_size(size);
    if initial { builder.with_position(corner) } else { builder }
}

impl App {
    /// What is going on, most important first: a call, a transfer, then music playing.
    fn activity(&self) -> Option<Activity> {
        if let (Some(call), Some(peer)) = (&self.call, self.peer()) {
            return Some(Activity::Call { peer, call: call.clone() });
        }
        let transfer = self.island_transfers.activity(self.island_linger);
        if matches!(&transfer, Some(Activity::Transfer { state, .. }) if !state.is_finished()) {
            return transfer;
        }
        for (id, players) in &self.players {
            if let (Ok(peer), Some(player)) = (id.parse::<DeviceId>(), players.iter().find(|p| p.playing)) {
                let art = player.art_key.as_ref().and_then(|k| self.art.get(k)).cloned();
                return Some(Activity::Music { phone: Some(peer), player: player.clone(), art });
            }
        }
        if let Some(player) = crate::pc_media::last().into_iter().find(|p| p.playing) {
            return Some(Activity::Music { phone: None, player, art: None });
        }
        transfer
    }

    /// What is going on, or for a while after it stopped (Preferences › Window): paused music
    /// stays with its play button, a finished transfer as it ended. Calls go at once.
    fn activity_or_last(&mut self) -> Option<Activity> {
        let now = self.activity();
        if let Some(a) = &now {
            self.island_last = if matches!(a, Activity::Transfer { .. }) {
                None
            } else {
                Some((a.clone(), std::time::Instant::now()))
            };
            return now;
        }
        let (last, at) = self.island_last.clone()?;
        if at.elapsed().as_secs() >= u64::from(self.island_linger) {
            self.island_last = None;
            return None;
        }
        match last {
            Activity::Call { .. } => None,
            // The same player as it is now (paused), so its button plays again.
            Activity::Music { phone, player, art } => {
                let current = match phone {
                    Some(peer) => self.players.get(&peer.to_string()).cloned().unwrap_or_default(),
                    None => crate::pc_media::last(),
                };
                let player = current.into_iter().find(|p| p.id == player.id)?;
                Some(Activity::Music { phone, player, art })
            }
            Activity::Transfer { .. } => None,
        }
    }

    /// Every frame of the main window: shows the island while something is going on.
    pub(crate) fn island_window(&mut self, ctx: &egui::Context) {
        if !self.island_on {
            self.island = None;
            return;
        }
        // This PC's music has no event of its own: look again now and then.
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
        let Some(node) = self.node.clone() else { return };
        let island = self
            .island
            .get_or_insert_with(|| Island::new(node, self.rt.handle().clone(), ctx, self.tx.clone()))
            .clone();
        let activity = self.activity_or_last();
        let showing = activity.is_some();
        let urgent = activity.as_ref().is_some_and(Activity::urgent);
        *island.activity.lock().unwrap() = activity;
        if !showing {
            // Not shown this frame: egui closes the window. It opens small next time.
            island.large.store(false, Ordering::Release);
            island.shown.store(false, Ordering::Release);
            *island.shaped.lock().unwrap() = (0, 0);
            return;
        }
        let size = if island.large.load(Ordering::Acquire) { LARGE } else { SMALL };
        let initial = !island.shown.swap(true, Ordering::AcqRel);
        let corner = if initial { island.corner(ctx.pixels_per_point(), size) } else { egui::Pos2::ZERO };
        ctx.show_viewport_deferred(
            Island::viewport(),
            viewport_builder(corner, size, urgent, initial),
            move |ctx, _| island.show(ctx),
        );
    }
}

/// Cuts the island's window to a rounded pill (X11 shape) each time its size changes, and
/// tells whether the pointer is on it (asked every 150 ms).
fn shape_thread(
    rx: &mpsc::Receiver<(u16, u16)>,
    hovered: &AtomicBool,
    ctx: &egui::Context,
) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::shape::{self, ConnectionExt as _};
    use x11rb::protocol::xproto::{
        Arc as XArc, AtomEnum, ConnectionExt as _, CreateGCAux, Rectangle, Window,
    };

    // Closed before the first size: the island was never shown.
    let Ok(mut size) = rx.recv() else { return Ok(()) };
    let (conn, screen) = x11rb::rust_connection::RustConnection::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let find = |w: Window| -> Option<Window> {
        let mut stack = vec![w];
        while let Some(w) = stack.pop() {
            let name =
                conn.get_property(false, w, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 64).ok()?.reply().ok()?;
            if name.value == TITLE.as_bytes() {
                return Some(w);
            }
            stack.extend(conn.query_tree(w).ok()?.reply().ok()?.children);
        }
        None
    };
    loop {
        // The window may not exist yet right after it is asked for.
        let mut window = None;
        for _ in 0..20 {
            window = find(root);
            if window.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if let Some(win) = window {
            let (w, h) = size;
            let r = (h / 2).min(22);
            let mask = conn.generate_id()?;
            conn.create_pixmap(1, mask, win, w, h)?;
            let gc = conn.generate_id()?;
            conn.create_gc(gc, mask, &CreateGCAux::new().foreground(0))?;
            conn.poly_fill_rectangle(mask, gc, &[Rectangle { x: 0, y: 0, width: w, height: h }])?;
            conn.change_gc(gc, &x11rb::protocol::xproto::ChangeGCAux::new().foreground(1))?;
            let (wi, hi, ri) = (w as i16, h as i16, r as i16);
            conn.poly_fill_rectangle(
                mask,
                gc,
                &[
                    Rectangle { x: ri, y: 0, width: w - 2 * r, height: h },
                    Rectangle { x: 0, y: ri, width: w, height: h - 2 * r },
                ],
            )?;
            let corner =
                |x: i16, y: i16| XArc { x, y, width: 2 * r, height: 2 * r, angle1: 0, angle2: 360 * 64 };
            conn.poly_fill_arc(
                mask,
                gc,
                &[
                    corner(0, 0),
                    corner(wi - 2 * ri, 0),
                    corner(0, hi - 2 * ri),
                    corner(wi - 2 * ri, hi - 2 * ri),
                ],
            )?;
            conn.shape_mask(shape::SO::SET, shape::SK::BOUNDING, win, 0, 0, mask)?;
            conn.free_pixmap(mask)?;
            conn.free_gc(gc)?;
            conn.flush()?;
        }
        // Until the next size: follow the pointer.
        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(150)) {
                Ok(next) => {
                    size = next;
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let on = window.is_some_and(|win| {
                conn.query_pointer(win).ok().and_then(|c| c.reply().ok()).is_some_and(|p| {
                    p.same_screen
                        && (0..i32::from(size.0)).contains(&i32::from(p.win_x))
                        && (0..i32::from(size.1)).contains(&i32::from(p.win_y))
                })
            });
            if hovered.swap(on, Ordering::AcqRel) != on {
                ctx.request_repaint_of(Island::viewport());
            }
        }
    }
}

/// Whether X11 window `win` is out of sight: minimized (`_NET_WM_STATE_HIDDEN`) or hidden in
/// the tray (unmapped). egui isn't told under XWayland.
pub(crate) fn window_hidden(win: u32) -> bool {
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, MapState};
    let Some((conn, _)) = x11() else { return false };
    // Closing with × hides the window in the tray (unmapped): no _NET_WM_STATE_HIDDEN then.
    if conn
        .get_window_attributes(win)
        .ok()
        .and_then(|c| c.reply().ok())
        .is_some_and(|a| a.map_state != MapState::VIEWABLE)
    {
        return true;
    }
    let atom = |name: &[u8]| conn.intern_atom(false, name).ok()?.reply().ok().map(|r| r.atom);
    let (Some(state), Some(hidden)) = (atom(b"_NET_WM_STATE"), atom(b"_NET_WM_STATE_HIDDEN")) else {
        return false;
    };
    conn.get_property(false, win, state, AtomEnum::ATOM, 0, 32)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().map(|mut v| v.any(|a| a == hidden)))
        .unwrap_or(false)
}

/// One X11 connection and its root window, kept for the drags and the screen's size.
fn x11() -> Option<&'static (x11rb::rust_connection::RustConnection, u32)> {
    use x11rb::connection::Connection;
    use x11rb::rust_connection::RustConnection;
    static CONN: std::sync::OnceLock<Option<(RustConnection, u32)>> = std::sync::OnceLock::new();
    CONN.get_or_init(|| {
        let (conn, screen) = RustConnection::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        Some((conn, root))
    })
    .as_ref()
}

/// Where the pointer is on the screen, in pixels.
fn screen_pointer() -> Option<(f32, f32)> {
    use x11rb::protocol::xproto::ConnectionExt as _;
    let (conn, root) = x11()?;
    let p = conn.query_pointer(*root).ok()?.reply().ok()?;
    Some((p.root_x as f32, p.root_y as f32))
}

/// The main monitor's area and the whole screen's size, in pixels.
fn screen_area() -> Option<(egui::Rect, egui::Vec2)> {
    use x11rb::protocol::xproto::ConnectionExt as _;
    let (conn, root) = x11()?;
    let g = conn.get_geometry(*root).ok()?.reply().ok()?;
    let (x, y, w, h) = crate::laser::primary(conn, *root, (g.width, g.height));
    Some((
        egui::Rect::from_min_size(egui::pos2(x as f32, y as f32), egui::vec2(w as f32, h as f32)),
        egui::vec2(g.width as f32, g.height as f32),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
        fn visit(shape: &egui::epaint::Shape, result: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::epaint::Shape::Text(text) => result.push((
                    text.galley.text().to_owned(),
                    egui::Rect::from_min_size(text.pos, text.galley.size()),
                )),
                egui::epaint::Shape::Vec(shapes) => {
                    for shape in shapes {
                        visit(shape, result);
                    }
                }
                _ => {}
            }
        }
        let mut result = vec![];
        for shape in shapes {
            visit(&shape.shape, &mut result);
        }
        result
    }

    #[test]
    fn compact_transfer_keeps_name_and_percentage_inside_and_centered() {
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SMALL)),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 6)))
                    .show(ctx, |ui| {
                        transfer(
                            ui,
                            &Activity::Transfer {
                                name: "A long example file name that must leave room for the percentage.bin"
                                    .into(),
                                more: 0,
                                done: 50,
                                total: 100,
                                direction: Some(broche_core::Direction::Outgoing),
                                state: TransferState::Running,
                            },
                            false,
                        );
                    });
            },
        );
        let texts = text_rects(&output.shapes);
        let name = texts.iter().find(|(text, _)| text.starts_with("A long")).expect("filename").1;
        let percent = texts.iter().find(|(text, _)| text == "50 %").expect("percentage").1;
        assert!(percent.right() <= SMALL.x - 16.0 + 0.1, "percentage outside island: {percent:?}");
        assert!(name.right() + 4.0 <= percent.left(), "filename overlaps percentage: {name:?}, {percent:?}");
        assert!(
            (name.center().y - percent.center().y).abs() <= 1.0,
            "different text centers: {name:?}, {percent:?}"
        );
        assert!((name.center().y - SMALL.y / 2.0).abs() <= 1.0, "filename not centered: {name:?}");
    }

    #[test]
    fn expanded_call_keeps_controls_inside_and_leaves_room_for_audio() {
        let call = CallState {
            id: "example-call".into(),
            state: "active".into(),
            incoming: false,
            number: None,
            name: Some("A long example caller name that must not push controls outside".into()),
            photo: None,
            missed: false,
            since: None,
            controls: None,
        };
        let ctx = egui::Context::default();
        let mut remaining = 0.0;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, LARGE)),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 6)))
                    .show(ctx, |ui| {
                        let _ = call_content(ui, &call, true);
                        remaining = ui.available_height();
                    });
            },
        );
        for (text, rect) in text_rects(&output.shapes) {
            assert!(
                rect.left() >= 16.0 && rect.right() <= LARGE.x - 16.0 + 0.1,
                "call text outside: {text}, {rect:?}"
            );
        }
        assert!(remaining >= 22.0, "no room for the call audio button: {remaining}");
    }

    #[test]
    fn expanded_transfer_centers_the_text_and_progress_block() {
        let ctx = egui::Context::default();
        let mut spacing = 0.0;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, LARGE)),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 6)))
                    .show(ctx, |ui| {
                        spacing = ui.spacing().item_spacing.y;
                        transfer(
                            ui,
                            &Activity::Transfer {
                                name: "Example file".into(),
                                more: 0,
                                done: 460,
                                total: 1000,
                                direction: Some(broche_core::Direction::Outgoing),
                                state: TransferState::Running,
                            },
                            true,
                        );
                    });
            },
        );
        let texts = text_rects(&output.shapes);
        let title = texts.iter().find(|(text, _)| text == "Example file").expect("title").1;
        let bytes = texts.iter().find(|(text, _)| text.contains("460 B / 1000 B")).expect("bytes").1;
        let centre = (title.top() + bytes.bottom() + spacing + 6.0) / 2.0;
        assert!((centre - LARGE.y / 2.0).abs() <= 3.0, "details are not centered: {centre}");
    }

    fn report(id: &str, done: u64, total: u64, state: TransferState) -> Transfer {
        Transfer {
            id: id.into(),
            device: DeviceId([7; 32]),
            direction: broche_core::Direction::Outgoing,
            names: vec![format!("{id}.bin")],
            files: 1,
            total,
            done,
            state,
            open_on_arrival: false,
        }
    }

    #[test]
    fn progress_is_weighted_and_keeps_completed_members_until_the_group_ends() {
        let mut progress = Transfers::default();
        progress.update(&report("small", 10, 100, TransferState::Running));
        progress.update(&report("large", 450, 900, TransferState::Running));
        let Some(Activity::Transfer { done, total, .. }) = progress.activity(15) else {
            panic!("transfer summary")
        };
        assert_eq!((done, total), (460, 1000));
        progress.update(&report("small", 100, 100, TransferState::Done { saved: vec![] }));
        let Some(Activity::Transfer { done, total, .. }) = progress.activity(15) else {
            panic!("transfer summary")
        };
        assert_eq!((done, total), (550, 1000));
        progress.update(&report("large", 900, 900, TransferState::Done { saved: vec![] }));
        let Some(Activity::Transfer { done, total, state, .. }) = progress.activity(15) else {
            panic!("finished summary")
        };
        assert_eq!((done, total), (1000, 1000));
        assert!(matches!(state, TransferState::Done { .. }));
        assert!(progress.activity(0).is_none());
        progress.update(&report("next", 0, 500, TransferState::Waiting));
        let Some(Activity::Transfer { done, total, .. }) = progress.activity(15) else {
            panic!("new summary")
        };
        assert_eq!((done, total), (0, 500));
    }

    #[test]
    fn queued_uploads_keep_failures_and_completed_bytes_in_the_same_group() {
        let mut progress = Transfers::default();
        progress.update(&report("first", 0, 100, TransferState::Waiting));
        progress.update(&report("second", 0, 900, TransferState::Waiting));
        progress.update(&report(
            "first",
            40,
            100,
            TransferState::Failed(broche_core::TransferFailure::Unreachable),
        ));
        progress.update(&report("second", 900, 900, TransferState::Done { saved: vec![] }));
        let Some(Activity::Transfer { done, total, state, .. }) = progress.activity(15) else {
            panic!("failed group")
        };
        assert_eq!((done, total), (940, 1000));
        assert!(matches!(state, TransferState::Failed(_)));
        progress.update(&report("first", 40, 100, TransferState::Paused));
        let Some(Activity::Transfer { state, .. }) = progress.activity(15) else { panic!("resumed group") };
        assert_eq!(state, TransferState::Paused);
    }

    #[test]
    fn fullscreen_layering_does_not_treat_music_as_an_urgent_notification() {
        let normal = viewport_builder(egui::Pos2::ZERO, SMALL, false, true);
        assert_eq!(normal.window_type, Some(egui::X11WindowType::Dock));
        assert_eq!(normal.window_level, Some(egui::WindowLevel::Normal));
        assert_eq!(normal.active, Some(false));
        let ringing = viewport_builder(egui::Pos2::ZERO, LARGE, true, true);
        assert_eq!(ringing.window_type, Some(egui::X11WindowType::Dock));
        assert_eq!(ringing.window_level, Some(egui::WindowLevel::AlwaysOnTop));
    }

    #[test]
    fn dragging_freezes_hover_size_and_does_not_reapply_a_stale_builder_position() {
        assert!(!expanded(true, false, true, false));
        assert!(expanded(false, false, true, true));
        assert!(expanded(true, false, false, false));
        assert!(viewport_builder(egui::pos2(100.0, 100.0), LARGE, false, true).position.is_some());
        assert!(viewport_builder(egui::pos2(100.0, 100.0), LARGE, false, false).position.is_none());
    }

    #[test]
    fn grab_offset_stays_in_physical_pixels_when_display_scale_changes() {
        let pointer = egui::pos2(360.0, 180.0);
        let grab = egui::vec2(20.0, 10.0);
        assert_eq!(dragged_corner(pointer, grab, 1.0), egui::pos2(340.0, 170.0));
        assert_eq!(dragged_corner(pointer, grab, 2.0), egui::pos2(170.0, 85.0));
    }

    #[test]
    fn direct_downloads_and_native_transfers_are_not_double_counted() {
        let mut downloads = crate::Downloads::default();
        let mut progress = Transfers::default();
        let id = downloads.add("example.txt".into(), 100, DeviceId([7; 32]));
        downloads.progress(id, 40, 100);
        progress.download(downloads.generation, id, &downloads.items[id]);
        progress.download(downloads.generation, id, &downloads.items[id]);
        progress.update(&report("native", 60, 300, TransferState::Running));
        let Some(Activity::Transfer { done, total, direction, .. }) = progress.activity(15) else {
            panic!("mixed group")
        };
        assert_eq!((done, total), (100, 400));
        assert_eq!(direction, None);
        downloads.finish(id, Ok(()));
        progress.download(downloads.generation, id, &downloads.items[id]);
        progress.update(&report("native", 300, 300, TransferState::Done { saved: vec![] }));
        let next = downloads.add("empty.txt".into(), 0, DeviceId([7; 32]));
        assert_eq!(next, id);
        progress.download(downloads.generation, next, &downloads.items[next]);
        downloads.finish(next, Ok(()));
        progress.download(downloads.generation, next, &downloads.items[next]);
        let Some(Activity::Transfer { done, total, state, .. }) = progress.activity(15) else {
            panic!("new empty group")
        };
        assert_eq!((done, total), (0, 0));
        assert!(matches!(state, TransferState::Done { .. }));
    }
}
