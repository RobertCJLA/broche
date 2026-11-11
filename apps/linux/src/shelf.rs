// SPDX-License-Identifier: GPL-3.0-or-later
//! The shelf: a panel at the right edge of the screen, above other windows, with the phone's
//! latest photo and screenshot, its latest clip, and the files it sent lately, as the original
//! NectarLink's Shelf. Files dropped on it go to the phone; received files drag out into
//! other apps (`xdnd`). It is a deferred egui viewport with the node of its own, so it works
//! while the main window is minimized.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use broche_core::{DeviceId, Node};
use eframe::egui::{self, RichText};
use tokio::{runtime::Handle, sync::mpsc};

use crate::{Msg, i18n::t, kit};

const WIDTH: f32 = 348.0;
/// Received files listed, newest first.
const RECENT: usize = 6;

/// A received file and its size.
type Received = (PathBuf, u64);

/// One open shelf.
pub struct Shelf {
    pub peer: DeviceId,
    phone: String,
    node: Node,
    rt: Handle,
    ui: mpsc::UnboundedSender<Msg>,
    pub closed: AtomicBool,
    photos: Mutex<[Option<Photo>; 2]>,
    received: Mutex<(Vec<Received>, Option<Instant>)>,
}

/// The newest picture of an album, with its thumbnail once it arrives.
#[derive(Clone)]
struct Photo {
    id: String,
    tex: Option<egui::TextureHandle>,
}

/// Albums for the two picture slots, by the names phones give them.
fn slot_of(album: &str) -> Option<usize> {
    let a = album.to_lowercase();
    if a == "camera" || a == "cámara" || a == "dcim" {
        Some(0)
    } else if a.contains("screenshot") || a.contains("captura") {
        Some(1)
    } else {
        None
    }
}

impl Shelf {
    pub fn open(
        peer: DeviceId,
        phone: String,
        node: Node,
        rt: Handle,
        ui: mpsc::UnboundedSender<Msg>,
        ctx: &egui::Context,
    ) -> Arc<Self> {
        let shelf = Arc::new(Self {
            peer,
            phone,
            node,
            rt,
            ui,
            closed: AtomicBool::new(false),
            photos: Mutex::new([None, None]),
            received: Mutex::new((vec![], None)),
        });
        shelf.load_photos(ctx);
        shelf
    }

    fn viewport() -> egui::ViewportId {
        egui::ViewportId::from_hash_of("shelf")
    }

    /// Asks the phone for its albums' newest pictures, then their thumbnails.
    fn load_photos(self: &Arc<Self>, ctx: &egui::Context) {
        let (me, ctx) = (self.clone(), ctx.clone());
        self.rt.spawn(async move {
            let Ok(albums) = me.node.photo_albums(me.peer).await else { return };
            let mut ids: [Option<String>; 2] = [None, None];
            for album in albums {
                if let (Some(slot), Some(cover)) = (slot_of(&album.name), album.cover) {
                    ids[slot].get_or_insert(cover);
                }
            }
            *me.photos.lock().unwrap() = ids.clone().map(|id| id.map(|id| Photo { id, tex: None }));
            ctx.request_repaint_of(Self::viewport());
            let wanted: Vec<String> = ids.into_iter().flatten().collect();
            let Ok(thumbs) = me.node.photo_thumbs(me.peer, wanted).await else { return };
            for th in thumbs {
                let Ok(img) = image::load_from_memory(&th.data) else { continue };
                let rgba = img.to_rgba8();
                let ci = egui::ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as usize, rgba.height() as usize],
                    rgba.as_raw(),
                );
                let tex = ctx.load_texture(format!("shelf-{}", th.id), ci, Default::default());
                for p in me.photos.lock().unwrap().iter_mut().flatten() {
                    if p.id == th.id {
                        p.tex = Some(tex.clone());
                    }
                }
            }
            ctx.request_repaint_of(Self::viewport());
        });
    }

    /// The newest files in Recibidos, read again every couple of seconds while open.
    fn received(&self) -> Vec<Received> {
        let mut cache = self.received.lock().unwrap();
        if cache.1.is_none_or(|at| at.elapsed() > Duration::from_secs(2)) {
            let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> =
                std::fs::read_dir(crate::received_dir())
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|e| {
                        let meta = e.metadata().ok().filter(|m| m.is_file())?;
                        Some((e.path(), meta.len(), meta.modified().ok()?))
                    })
                    .collect();
            files.sort_by_key(|f| std::cmp::Reverse(f.2));
            *cache = (files.into_iter().take(RECENT).map(|(p, s, _)| (p, s)).collect(), Some(Instant::now()));
        }
        cache.0.clone()
    }

    fn close(&self, ctx: &egui::Context) {
        self.closed.store(true, Ordering::Release);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    fn line(&self, text: String) {
        let _ = self.ui.send(Msg::Line(text));
    }

    /// Draws the shelf: called by egui for its viewport.
    fn show(self: &Arc<Self>, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape)) {
            self.close(ctx);
            return;
        }
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            self.send(dropped);
        }
        ctx.request_repaint_after(Duration::from_secs(2));
        let p = crate::theme::palette_for(crate::theme::current_scheme(), ctx.style().visuals.dark_mode);
        let frame = egui::Frame::NONE
            .fill(p.surface)
            .inner_margin(14.0)
            .stroke(egui::Stroke::new(1.0_f32, p.outline_variant));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            // Dragging a file must start a drag, not scroll the list.
            egui::ScrollArea::vertical()
                .scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() })
                .show(ui, |ui| self.contents(ui, &p));
        });
    }

    fn contents(self: &Arc<Self>, ui: &mut egui::Ui, p: &crate::theme::Palette) {
        {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(t("Shelf")).strong().size(18.0));
                    ui.label(RichText::new(&self.phone).small().color(p.on_surface_variant));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if kit::icon_button(ui, "✖", &t("Close")).clicked() {
                        self.close(ui.ctx());
                    }
                    if kit::icon_button(ui, "⟳", &t("Refresh")).clicked() {
                        self.received.lock().unwrap().1 = None;
                        self.load_photos(ui.ctx());
                    }
                });
            });
            ui.add_space(8.0);
            kit::card(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(format!("{} {}", t("Drop files here to send them to"), self.phone));
                });
            });
            ui.add_space(6.0);
            let photos = self.photos.lock().unwrap().clone();
            ui.columns(2, |cols| {
                for (i, (ui, title)) in
                    cols.iter_mut().zip([t("Latest photo"), t("Latest screenshot")]).enumerate()
                {
                    kit::card(ui, |ui| {
                        ui.label(RichText::new(title).small().strong());
                        let size = egui::vec2(ui.available_width(), 96.0);
                        match photos[i].as_ref() {
                            Some(photo) => {
                                if let Some(tex) = &photo.tex {
                                    // Fill the slot and crop the rest, like the Photos page.
                                    let s = tex.size_vec2();
                                    let (want, have) = (size.x / size.y, s.x / s.y.max(1.0));
                                    let uv = if have > want {
                                        let u = (1.0 - want / have) / 2.0;
                                        egui::Rect::from_min_max(egui::pos2(u, 0.0), egui::pos2(1.0 - u, 1.0))
                                    } else {
                                        let v = (1.0 - have / want) / 2.0;
                                        egui::Rect::from_min_max(egui::pos2(0.0, v), egui::pos2(1.0, 1.0 - v))
                                    };
                                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                    egui::Image::new(tex).uv(uv).corner_radius(8).paint_at(ui, rect);
                                } else {
                                    ui.add_sized(size, egui::Spinner::new());
                                }
                                if kit::tonal_button(ui, &t("Save")).clicked() {
                                    self.fetch(photo.id.clone());
                                }
                            }
                            None => {
                                ui.add_sized(
                                    size,
                                    egui::Label::new(RichText::new(t("None")).color(p.on_surface_variant)),
                                );
                            }
                        }
                    });
                }
            });
            ui.add_space(6.0);
            if let Some(clip) =
                self.node.clipboard_history(None).into_iter().next().filter(|c| !c.text.is_empty())
            {
                kit::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("Latest clip")).small().strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if kit::tonal_button(ui, &t("Copy")).clicked() {
                                ui.ctx().copy_text(clip.text.clone());
                            }
                        });
                    });
                    ui.add(egui::Label::new(RichText::new(&clip.text).monospace()).truncate());
                });
                ui.add_space(6.0);
            }
            kit::card(ui, |ui| {
                ui.label(RichText::new(t("Received lately")).small().strong());
                kit::supporting(ui, t("Drag a file into another app, or open it."));
                let files = self.received();
                if files.is_empty() {
                    kit::supporting(ui, t("Nothing yet."));
                }
                for (path, size) in files {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    // The buttons first, from the right; the file takes the rest of the row.
                    let row_size = egui::vec2(ui.available_width(), 44.0);
                    ui.allocate_ui_with_layout(
                        row_size,
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if kit::icon_button(ui, "📁", &t("Show in folder")).clicked() {
                                crate::tray::open_folder(path.parent().unwrap_or(&path));
                            }
                            let open = kit::tonal_button(ui, &t("Open")).clicked();
                            let row = ui.add_sized(
                                [ui.available_width(), 44.0],
                                egui::Button::new(format!("{name}\n{}", crate::human_size(size)))
                                    .truncate()
                                    .sense(egui::Sense::click_and_drag()),
                            );
                            if row.drag_started() {
                                crate::xdnd::start(vec![path.clone()]);
                            }
                            // The pointer stays this window's while dragging out: show it carries something.
                            if row.dragged() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            }
                            if open || row.clicked() {
                                crate::tray::open_file(&path);
                            }
                        },
                    );
                }
            });
        }
    }

    /// Files dropped on the shelf go to the phone.
    fn send(&self, paths: Vec<PathBuf>) {
        let (node, peer, ui) = (self.node.clone(), self.peer, self.ui.clone());
        self.rt.spawn(async move {
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

    /// A picture from the phone, saved to Recibidos (where it can then be dragged).
    fn fetch(&self, id: String) {
        let (node, peer) = (self.node.clone(), self.peer);
        self.line(t("Saving the picture to Recibidos…"));
        self.rt.spawn(async move {
            let _ = node.fetch_photo(peer, id).await;
        });
    }
}

impl crate::App {
    /// Opens the shelf, or closes it if it is open.
    pub(crate) fn toggle_shelf(&mut self, ctx: &egui::Context) {
        if let Some(shelf) = self.shelf.take() {
            shelf.close(ctx);
            return;
        }
        let (Some(peer), Some(node)) = (self.peer(), self.node.clone()) else {
            self.log.push(t("Pair a phone first."));
            return;
        };
        let phone = self
            .devices
            .iter()
            .find(|d| d.id == peer.to_string())
            .map(|d| d.name.clone())
            .unwrap_or_default();
        self.shelf = Some(Shelf::open(peer, phone, node, self.rt.handle().clone(), self.tx.clone(), ctx));
    }

    /// Every frame of the main window: keeps the shelf open, at the right edge of the screen.
    pub(crate) fn shelf_window(&mut self, ctx: &egui::Context) {
        if self.shelf.as_ref().is_some_and(|s| s.closed.load(Ordering::Acquire)) {
            self.shelf = None;
        }
        let Some(shelf) = self.shelf.clone() else { return };
        let screen = ctx.input(|i| i.viewport().monitor_size).unwrap_or(egui::vec2(1920.0, 1080.0));
        let height = (screen.y - 96.0).clamp(420.0, 640.0);
        let pos = egui::pos2(screen.x - WIDTH - 12.0, ((screen.y - height) / 2.0).max(0.0));
        ctx.show_viewport_deferred(
            Shelf::viewport(),
            egui::ViewportBuilder::default()
                .with_title(t("Shelf"))
                .with_app_id("io.github.robertcjla.Broche")
                .with_decorations(false)
                .with_window_level(egui::WindowLevel::AlwaysOnTop)
                .with_taskbar(false)
                .with_resizable(false)
                .with_position(pos)
                .with_inner_size([WIDTH, height]),
            move |ctx, _| shelf.show(ctx),
        );
    }
}
