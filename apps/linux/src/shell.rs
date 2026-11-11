// SPDX-License-Identifier: GPL-3.0-or-later
//! The layout of the app: a Material navigation drawer (or a rail when the window is
//! narrow), a page header with tabs, the Home page with the things people do most,
//! and Preferences for everything that is set once.

use broche_core::{ConnectionPath, DeviceId, LinkState, MediaAction};
use eframe::egui::{self, CornerRadius, Margin, RichText, Sense, Stroke, ThemePreference};

use crate::{
    App, Device, Pick, Tab, deck, deps, i18n,
    i18n::{Lang, t},
    kit::{self, card, dot, supporting, title},
    media, received_dir,
    settings::{self, Settings},
    theme::{self, RADIUS_FULL, Scheme, pal},
};

/// Below this width the side bar becomes a rail of icons.
/// Width the phone picker needs next to a page's headline (with a gap).
const DEVICE_PICKER_ROOM: f32 = 200.0;

/// Tips shown one at a time on Home, picked at random each time Broche opens (keys for `t`).
pub(crate) const TIPS: &[&str] = &[
    "Copy on the PC and paste on the phone: turn on Automatic clipboard in Settings › Preferences.",
    "With the window closed, the tray icon still shows the phone's screen, sends files and makes it ring.",
    "Can't find your phone? Find my phone makes it ring, even when it's silent.",
    "See the phone's camera in Screen and camera › Camera, and save a photo or a scanned document on this PC.",
    "With advanced mode on the phone, open its apps in windows of their own from Screen and camera › Phone apps.",
    "Files from the phone go to Broche › Recibidos in your home folder.",
    "Drop files on this window or on the shelf to send them to the phone.",
    "To put a picture from the phone on this PC's clipboard, share it to Copy to PC clipboard.",
    "Reply to the phone's messages and notifications from this PC.",
    "Show the phone's notifications as desktop notifications too, in Settings › Preferences.",
    "The island at the top of the screen shows calls, transfers and music. Move it or turn it off in Settings › Preferences.",
    "No Wi-Fi? Connect the phone with a USB cable and USB debugging on, and Broche works through it.",
    "If the phone doesn't connect, Settings › Device checks the connection and says why.",
    "Everything the phone and this PC sent each other is in Files and photos › History.",
    "Press Ctrl+K to search pages, settings and actions, and run them from the keyboard.",
    "Ctrl + and Ctrl − change the text size on any page.",
    "Make your own PC shortcuts for the phone in Settings › Preferences.",
    "Choose what the phone may do on this PC in Settings › Device.",
    "Pair your phone with this PC in Bluetooth settings to hear and answer calls on the PC.",
];

/// A tip to start with: any of them, different from one start to the next.
pub(crate) fn random_tip() -> usize {
    let nanos =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    nanos as usize % TIPS.len()
}

/// From this content width, Home uses two columns.
const TWO_COLUMNS_FROM: f32 = 900.0;

/// The places in the side bar.
#[derive(PartialEq, Clone, Copy)]
pub enum Section {
    Home,
    Chat,
    Notifications,
    Media,
    Files,
    Settings,
}

/// Lower case without accents, so «ventana» finds «Ventana» and «compatibilidad» «compatibilidad».
pub(crate) fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// Every settings card: its title (where the search jumps), its tab and the words inside it.
pub(crate) fn settings_index() -> Vec<(String, Tab, Vec<String>)> {
    let mut index = vec![
        (
            t("Appearance"),
            Tab::Perms,
            vec![
                t("Mode"),
                t("Light"),
                t("Dark"),
                t("Follow the system"),
                t("Colour scheme"),
                t("Text size"),
            ],
        ),
        (t("Window"), Tab::Perms, vec![t("Tips on Home")]),
        (t("Language"), Tab::Perms, vec![]),
        (
            t("PC shortcuts on the phone"),
            Tab::Perms,
            vec![t("Open an app"), t("Open a link"), t("Run a command"), t("Add button")],
        ),
        (
            t("Startup and closing"),
            Tab::Perms,
            vec![
                t("Start with my session"),
                t("Keep running when the window is closed"),
                t("Minimize"),
                t("Quit application"),
            ],
        ),
        (
            t("Files"),
            Tab::Perms,
            vec![t("Folder your phone can browse on this PC:"), t("Files you receive are saved in:")],
        ),
        (t("Clipboard"), Tab::Perms, vec![t("Automatic clipboard")]),
        (
            t("Bluetooth calls"),
            Tab::Perms,
            vec![t("Talk on this PC"), t("Use PC audio when answering"), "Bluetooth".into()],
        ),
        (
            t("Video quality"),
            Tab::Perms,
            vec![t("Normal · 720p, 30 fps"), t("High · 1080p, 60 fps (experimental)")],
        ),
        (t("This device"), Tab::Access, vec![t("Unpair this device"), t("Unpair…")]),
    ];
    for (label, about, keys) in crate::PERM_GROUPS {
        let mut words = vec![t(about)];
        words.extend(keys.iter().map(|k| t(&format!("perm:{k}"))));
        index.push((t(label), Tab::Access, words));
    }
    index
}

impl Section {
    pub(crate) const ALL: [(Section, &'static str, &'static str); 6] = [
        (Section::Home, "🏠", "Home"),
        (Section::Chat, "💬", "Messages and calls"),
        (Section::Notifications, "🔔", "Notifications"),
        (Section::Media, "📱", "Screen and camera"),
        (Section::Files, "📁", "Files and photos"),
        (Section::Settings, "⚙", "Settings"),
    ];

    fn of(tab: Tab) -> Section {
        match tab {
            Tab::Devices => Section::Home,
            Tab::Messages | Tab::Calls => Section::Chat,
            Tab::Notifications => Section::Notifications,
            Tab::Screen | Tab::Camera | Tab::Apps => Section::Media,
            Tab::Files | Tab::Photos | Tab::History => Section::Files,
            Tab::Pair | Tab::Perms | Tab::Access | Tab::About => Section::Settings,
        }
    }

    pub(crate) fn first(self) -> Tab {
        match self {
            Section::Home => Tab::Devices,
            Section::Chat => Tab::Messages,
            Section::Notifications => Tab::Notifications,
            Section::Media => Tab::Screen,
            Section::Files => Tab::Files,
            Section::Settings => Tab::Perms,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        Section::ALL.iter().find(|(s, ..)| *s == self).map_or("", |(_, _, n)| n)
    }

    /// The pages of a place, when it has more than one.
    pub(crate) fn pages(self) -> &'static [(Tab, &'static str)] {
        match self {
            Section::Chat => &[(Tab::Messages, "Messages"), (Tab::Calls, "Calls and contacts")],
            Section::Media => &[(Tab::Screen, "Screen"), (Tab::Camera, "Camera"), (Tab::Apps, "Phone apps")],
            Section::Files => &[(Tab::Files, "Files"), (Tab::Photos, "Photos"), (Tab::History, "History")],
            Section::Settings => &[
                (Tab::Perms, "Preferences"),
                (Tab::Access, "Device"),
                (Tab::Pair, "Pair"),
                (Tab::About, "About"),
            ],
            _ => &[],
        }
    }
}

impl App {
    pub(crate) fn go(&mut self, tab: Tab) {
        if self.tab != tab {
            self.tab = tab;
            self.on_enter();
        }
    }

    pub(crate) fn selected_device(&self) -> Option<Device> {
        self.devices.iter().find(|d| Some(&d.id) == self.selected.as_ref()).cloned()
    }

    /// The chosen theme and text size, when they changed (the app and the welcome).
    pub(crate) fn apply_look(&mut self, ctx: &egui::Context) {
        let look = (self.theme_pref, self.scheme, self.system_dark);
        if self.theme_applied != Some(look) {
            theme::apply(ctx, self.theme_pref, self.scheme);
            // "Follow the system": the portal's answer, when there is one, over winit's.
            if let (egui::ThemePreference::System, Some(dark)) = (self.theme_pref, self.system_dark) {
                ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
            }
            self.theme_applied = Some(look);
        }
        let dark = ctx.style().visuals.dark_mode;
        let rect = ctx.content_rect();
        theme::paint_backdrop(
            &ctx.layer_painter(egui::LayerId::background()),
            rect,
            &theme::palette_for(self.scheme, dark),
            dark,
        );
        if (ctx.zoom_factor() - self.zoom).abs() > 0.001 && !self.zoom_set {
            ctx.set_zoom_factor(self.zoom);
            self.zoom_set = true;
        }
    }

    /// The whole window.
    pub fn shell(&mut self, ctx: &egui::Context) {
        self.apply_look(ctx);
        self.banners(ctx);

        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(Margin::symmetric(16, 6)),
            )
            .show(ctx, |ui| {
                let last = self.log.last().cloned().unwrap_or_else(|| t("Ready"));
                ui.add(
                    egui::Label::new(RichText::new(last).small().color(pal(ui).on_surface_variant))
                        .truncate(),
                );
            });

        let section = Section::of(self.tab);
        let (nav_fill, nav_rim) = {
            let dark = ctx.style().visuals.dark_mode;
            let p = theme::palette_for(self.scheme, dark);
            (p.glass(dark), p.rim(dark))
        };
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(88.0)
            .frame(
                egui::Frame::new()
                    .fill(nav_fill)
                    .stroke(Stroke::new(1.0_f32, nav_rim))
                    .inner_margin(Margin::symmetric(8, 12)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.rail(ui, section));
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(Margin { left: 24, right: 24, top: 16, bottom: 8 })
                    .fill(ctx.style().visuals.panel_fill),
            )
            .show(ctx, |ui| {
                self.page_header(ui, section);
                self.file_progress(ui);
                ui.set_opacity(theme::appear(ui.ctx(), egui::Id::new("page-appear"), self.tab, 0.18));
                match self.tab {
                    Tab::Devices => {
                        egui::ScrollArea::vertical().show(ui, |ui| self.home_tab(ui));
                    }
                    Tab::Screen => self.screen_tab(ui, ctx),
                    Tab::Messages => self.messages_tab(ui),
                    Tab::Calls => self.calls_tab(ui),
                    Tab::Notifications => self.notifications_tab(ui),
                    Tab::Photos => self.photos_tab(ui),
                    Tab::History => self.history_tab(ui),
                    Tab::Apps => self.apps_tab(ui),
                    Tab::Camera => self.camera_tab(ui, ctx),
                    Tab::Files => self.files_tab(ui),
                    Tab::Pair => {
                        egui::ScrollArea::vertical().show(ui, |ui| self.pair_tab(ui));
                    }
                    Tab::Perms => {
                        egui::ScrollArea::vertical().show(ui, |ui| self.preferences_tab(ui));
                    }
                    Tab::Access => {
                        egui::ScrollArea::vertical().show(ui, |ui| self.access_tab(ui));
                    }
                    Tab::About => {
                        egui::ScrollArea::vertical().show(ui, |ui| self.about_tab(ui));
                    }
                }
            });
    }

    /// The navigation: icons with short names under them (as in the original NectarLink).
    fn rail(&mut self, ui: &mut egui::Ui, section: Section) {
        ui.vertical_centered(|ui| {
            self.logo_image(ui, 32.0);
        });
        ui.add_space(12.0);
        // The selected pill is drawn under the icons and glides to the new place.
        let slot = ui.painter().add(egui::Shape::Noop);
        let mut target = None;
        for (s, icon, label) in Section::ALL {
            let short = t(label);
            let count = if s == Section::Notifications { self.notifications.len() } else { 0 };
            let name = if count > 0 { format!("{short} ({count})") } else { short };
            let resp = nav_item(ui, section == s, icon, &name, count);
            if section == s {
                target = Some(nav_pill(resp.rect));
            }
            if resp.clicked() {
                self.go(s.first());
            }
            ui.add_space(4.0);
        }
        if let Some(pill) = target {
            let y = theme::glide(ui.ctx(), egui::Id::new("nav-pill"), pill.top(), 0.22);
            let pill = pill.translate(egui::vec2(0.0, y - pill.top()));
            let shape = egui::Shape::rect_filled(pill, CornerRadius::same(16), pal(ui).primary_container);
            ui.painter().set(slot, shape);
        }
    }

    pub(crate) fn logo_image(&mut self, ui: &mut egui::Ui, size: f32) {
        let tex = self.logo.get_or_insert_with(|| {
            let img = image::load_from_memory(include_bytes!("../data/icon-256.png")).map(|i| i.to_rgba8());
            let img = img.unwrap_or_else(|_| image::RgbaImage::new(1, 1));
            let ci = egui::ColorImage::from_rgba_unmultiplied(
                [img.width() as usize, img.height() as usize],
                img.as_raw(),
            );
            ui.ctx().load_texture("logo", ci, Default::default())
        });
        ui.add(egui::Image::new(&*tex).fit_to_exact_size(egui::vec2(size, size)).alt_text("Broche"));
    }

    /// The headline of the place, the phone and the tabs of its pages.
    fn page_header(&mut self, ui: &mut egui::Ui, section: Section) {
        let title = t(section.name());
        // The phone picker goes under the headline when both don't fit on one line.
        let title_w = egui::WidgetText::from(RichText::new(&title).size(26.0))
            .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body)
            .size()
            .x;
        let one_line = ui.available_width() >= title_w + DEVICE_PICKER_ROOM;
        ui.horizontal(|ui| {
            kit::headline(ui, &title);
            if one_line && !self.devices.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.device_combo(ui));
            }
        });
        if !one_line && !self.devices.is_empty() {
            self.device_combo(ui);
        }
        let pages = section.pages();
        if !pages.is_empty() {
            ui.add_space(4.0);
            egui::ScrollArea::horizontal().id_salt("tabs").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (tab, label) in pages {
                        if tab_item(ui, self.tab == *tab, &t(label)).clicked() {
                            self.go(*tab);
                        }
                    }
                });
            });
            let p = pal(ui);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, p.outline_variant);
        }
        if section == Section::Settings {
            self.settings_search(ui);
        }
        ui.add_space(12.0);
    }

    /// Finds a setting by any word of its card or its options, without minding accents or case,
    /// and jumps to it.
    fn settings_search(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        kit::search_field(ui, &mut self.settings_query, &t("Search settings"));
        let query = fold(self.settings_query.trim());
        if query.is_empty() {
            return;
        }
        let hits: Vec<(String, Tab)> = settings_index()
            .into_iter()
            .filter(|(title, _, words)| std::iter::once(title).chain(words).any(|w| fold(w).contains(&query)))
            .map(|(title, tab, _)| (title, tab))
            .collect();
        if hits.is_empty() {
            supporting(ui, t("No settings match."));
        }
        ui.horizontal_wrapped(|ui| {
            for (title, tab) in hits {
                let place = Section::Settings.pages().iter().find(|(p, _)| *p == tab).map(|(_, l)| t(l));
                let label = match place {
                    Some(place) => format!("{place} › {title}"),
                    None => title.clone(),
                };
                if kit::tonal_button(ui, &label).clicked() {
                    self.go(tab);
                    self.settings_query.clear();
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(kit::SCROLL_TO), title));
                }
            }
        });
    }

    /// Which phone this window is about.
    fn device_combo(&mut self, ui: &mut egui::Ui) {
        let current = self.selected_device();
        let text = current.as_ref().map_or_else(|| t("No paired devices."), |d| d.name.clone());
        let combo = egui::ComboBox::from_id_salt("device").selected_text(text).show_ui(ui, |ui| {
            for d in self.devices.clone() {
                let online = d.state.starts_with("online");
                let label = format!("{} · {}", d.name, t(if online { "Connected" } else { "Offline" }));
                if ui.selectable_label(self.selected.as_deref() == Some(d.id.as_str()), label).clicked() {
                    self.selected = Some(d.id.clone());
                }
            }
        });
        kit::named(combo.response, &t("Device"));
    }

    /// The first page: the device, and what you do with it every day.
    fn home_tab(&mut self, ui: &mut egui::Ui) {
        if self.node.is_none() {
            kit::tinted_card(ui, pal(ui).error_container, |ui| {
                ui.label(
                    RichText::new(t("Could not start the Broche core. See Activity."))
                        .strong()
                        .color(pal(ui).on_error_container),
                );
                ui.label(
                    RichText::new(t(
                        "Is another Broche (or `broche-cli run`) already using the same data folder?",
                    ))
                    .color(pal(ui).on_error_container),
                );
            });
            return;
        }
        let Some(dev) = self.selected_device() else {
            self.no_phone(ui);
            return;
        };
        let Ok(peer) = dev.id.parse::<DeviceId>() else { return };
        if ui.available_width() >= TWO_COLUMNS_FROM {
            ui.columns(2, |cols| {
                cols[0].spacing_mut().item_spacing.y = 12.0;
                cols[1].spacing_mut().item_spacing.y = 12.0;
                self.home_main(&mut cols[0], peer, &dev);
                self.home_side(&mut cols[1], peer, &dev);
            });
        } else {
            ui.spacing_mut().item_spacing.y = 12.0;
            self.home_main(ui, peer, &dev);
            self.home_side(ui, peer, &dev);
        }
    }

    /// Home, first column: the phone, quick actions and sending.
    fn home_main(&mut self, ui: &mut egui::Ui, peer: DeviceId, dev: &Device) {
        let online = dev.state.starts_with("online");
        let p = pal(ui);
        let battery = self.batteries.get(&dev.id).map(|b| (b.level, b.charging));
        kit::tinted_card(ui, if online { p.primary_container } else { p.surface_container_high }, |ui| {
            let fg = if online { p.on_primary_container } else { p.on_surface };
            ui.horizontal(|ui| {
                phone_hero(ui, online, battery);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new(&dev.name).size(22.0).strong().color(fg));
                    ui.horizontal(|ui| {
                        dot(ui, if online { p.success } else { p.outline });
                        ui.label(RichText::new(t(if online { "Connected" } else { "Offline" })).color(fg));
                        if let Some(b) = self.batteries.get(&dev.id) {
                            let icon = if b.charging { "⚡" } else { "🔋" };
                            let text = format!("{icon} {} %", b.level);
                            let tip = format!(
                                "{}: {} %{}",
                                t("Battery"),
                                b.level,
                                if b.charging { format!(" · {}", t("charging")) } else { String::new() }
                            );
                            ui.label(RichText::new(text).color(if b.level <= 15 && !b.charging {
                                p.error
                            } else {
                                fg
                            }))
                            .on_hover_text(tip);
                        }
                    });
                    if online {
                        phone_facts(ui, dev, fg);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if kit::tonal_button(ui, &t("Unpair…")).on_hover_text(t("Unpair this device")).clicked()
                    {
                        self.confirm_unpair = Some(peer);
                    }
                });
            });
            if !online {
                supporting(ui, t("Offline devices can't be controlled. Check both are on the same network."));
            }
            ui.add_space(8.0);
            ui.add_enabled_ui(online, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // One button: it rings, and while ringing it stops it.
                    let ringing = self.phone_ringing();
                    let label = if ringing {
                        format!("🔕 {}", t("Stop ringing"))
                    } else {
                        format!("🔔 {}", t("Find my phone"))
                    };
                    if kit::filled_button(ui, &label).clicked() {
                        self.toggle_ring(peer);
                    }
                    if kit::tonal_button(ui, &t("Show its screen")).clicked() {
                        self.go(Tab::Screen);
                    }
                    if kit::tonal_button(ui, &t("Use its camera")).clicked() {
                        self.go(Tab::Camera);
                    }
                    if kit::tonal_button(ui, &t("Shelf")).clicked() {
                        self.toggle_shelf(ui.ctx());
                    }
                    // The phone asked for it: without this only the phone (or GNOME's indicator) stops it.
                    if self.platform.sharing_screen()
                        && kit::danger_button(ui, &t("Stop sharing this PC's screen")).clicked()
                    {
                        self.platform.stop_video();
                    }
                });
            });
        });

        card(ui, |ui| {
            title(ui, &t("Send to the phone"));
            supporting(ui, t("They arrive in the phone's Download/Broche folder."));
            ui.add_space(6.0);
            // A large target that opens the file chooser (Wayland windows can't take dropped files).
            let p = pal(ui);
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 64.0), Sense::click());
            let hot = resp.hovered() || resp.has_focus();
            ui.painter().rect(
                rect,
                CornerRadius::same(16),
                if hot { p.secondary_container } else { p.surface_container },
                Stroke::new(if hot { 2.0_f32 } else { 1.5 }, if hot { p.primary } else { p.outline }),
                egui::StrokeKind::Inside,
            );
            let label = ui.painter().layout_no_wrap(
                t("Choose files to send"),
                egui::FontId::proportional(15.0),
                p.on_surface,
            );
            let total = 34.0 + label.size().x;
            let left = rect.center().x - total / 2.0;
            ui.painter().text(
                egui::pos2(left + 12.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                "📤",
                egui::FontId::proportional(22.0),
                p.primary,
            );
            ui.painter().galley(
                egui::pos2(left + 34.0, rect.center().y - label.size().y / 2.0),
                label,
                p.on_surface,
            );
            kit::feedback(ui, &resp, p.on_surface, CornerRadius::same(16));
            let name = t("Choose files to send");
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.pick(Pick::Send(peer));
            }
            ui.add_space(8.0);
            let (_, send, enter) = kit::field_with_button(
                ui,
                &mut self.clip_text,
                &t("Text for its clipboard"),
                &t("Send text"),
                false,
            );
            if (send || enter) && !self.clip_text.is_empty() {
                let text = std::mem::take(&mut self.clip_text);
                self.act("clipboard", move |n| async move {
                    n.send_clipboard(peer, text)
                        .await
                        .map(|_| t("Clipboard sent."))
                        .map_err(|e| e.to_string())
                });
            }
        });

        if !self.received.is_empty() {
            card(ui, |ui| {
                title(ui, &t("Received lately"));
                for r in self.received.iter().take(4) {
                    ui.add(egui::Label::new(RichText::new(r).small()).truncate());
                }
                if kit::tonal_button(ui, &t("Open folder")).clicked() {
                    let _ = std::process::Command::new("xdg-open").arg(received_dir()).spawn();
                }
            });
        }
    }

    /// Home, second column: the phone's quick settings and its music, in one card.
    fn home_side(&mut self, ui: &mut egui::Ui, peer: DeviceId, _dev: &Device) {
        if let Some(tg) = self.node.clone().and_then(|node| node.phone_toggles(peer)) {
            card(ui, |ui| self.toggles_ui(ui, peer, tg));
        }
        card(ui, |ui| self.music_card(ui, peer));
        self.tip_card(ui);
    }

    /// The phone's music like Android's media widget: the cover large on the left, the title
    /// and artist, and Previous, a large round Play/Pause and Next under them.
    fn music_card(&mut self, ui: &mut egui::Ui, peer: DeviceId) {
        let p = pal(ui);
        let player = self.players.get(&peer.to_string()).and_then(|p| p.first()).cloned();
        let playing = player.as_ref().is_some_and(|p| p.playing);
        let title = player.as_ref().and_then(|p| p.title.clone()).filter(|s| !s.is_empty());
        let mut action = None;
        let toggle =
            if playing { ("⏸", "Pause", MediaAction::Pause) } else { ("▶", "Play", MediaAction::Play) };
        // Previous, Play/Pause, Next: added left to right so Tab and screen readers go in that order.
        let controls = |ui: &mut egui::Ui, action: &mut Option<MediaAction>| {
            if kit::icon_button(ui, "⏮", &t("Previous")).clicked() {
                *action = Some(MediaAction::Previous);
            }
            let big = ui.add(
                egui::Button::new(RichText::new(toggle.0).size(24.0).color(p.surface))
                    .fill(p.on_surface)
                    .corner_radius(28)
                    .min_size(egui::vec2(56.0, 56.0)),
            );
            let name = t(toggle.1);
            big.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            if big.on_hover_text(t(toggle.1)).clicked() {
                *action = Some(toggle.2);
            }
            if kit::icon_button(ui, "⏭", &t("Next")).clicked() {
                *action = Some(MediaAction::Next);
            }
        };
        match (&player, title) {
            (Some(pl), Some(title)) => {
                let side = (ui.available_width() * 0.38).clamp(88.0, 132.0);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
                    match pl.art_key.as_ref().and_then(|k| self.art.get(k)) {
                        Some(tex) => {
                            // Fill the square and crop the rest, like a gallery.
                            let s = tex.size_vec2();
                            let (u, v) = if s.x > s.y {
                                ((1.0 - s.y / s.x) / 2.0, 0.0)
                            } else {
                                (0.0, (1.0 - s.x / s.y.max(1.0)) / 2.0)
                            };
                            let uv = egui::Rect::from_min_max(egui::pos2(u, v), egui::pos2(1.0 - u, 1.0 - v));
                            egui::Image::new(tex).uv(uv).corner_radius(16).paint_at(ui, rect);
                        }
                        None => {
                            ui.painter().rect_filled(rect, 16, p.surface_container_high);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "♪",
                                egui::FontId::proportional(side * 0.4),
                                p.on_surface_variant,
                            );
                        }
                    }
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), side),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.add(egui::Label::new(RichText::new(&title).size(20.0).strong()).truncate());
                            if let Some(artist) = pl.artist.as_deref().filter(|s| !s.is_empty()) {
                                ui.add(
                                    egui::Label::new(RichText::new(artist).color(p.on_surface_variant))
                                        .truncate(),
                                );
                            }
                            ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                                // Left to right inside (a right-aligned row would reverse the order).
                                let w = 40.0 + 56.0 + 40.0 + 2.0 * ui.spacing().item_spacing.x;
                                ui.allocate_ui_with_layout(
                                    egui::vec2(w, 56.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| controls(ui, &mut action),
                                );
                            });
                        },
                    );
                });
            }
            _ => {
                ui.horizontal(|ui| {
                    kit::glyph(ui, "♪", 18.0, p.on_surface_variant);
                    ui.label(
                        RichText::new(t("Nothing is playing on the phone.")).color(p.on_surface_variant),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if kit::icon_button(ui, "▶", &t("Play")).clicked() {
                            action = Some(MediaAction::Play);
                        }
                    });
                });
            }
        }
        if let Some(action) = action {
            let id = player.as_ref().map(|p| p.id.clone());
            if matches!(action, MediaAction::Play | MediaAction::Pause)
                && let Some(p) = self.players.get_mut(&peer.to_string()).and_then(|p| p.first_mut())
            {
                // Flip now; the phone's next media event confirms it.
                p.playing = action == MediaAction::Play;
            }
            self.act("media", move |n| async move { media(n, peer, action, id).await });
        }
        // Only when the phone is also paired with this PC in the system's Bluetooth settings.
        if let Some(phone) = crate::bt_music::current() {
            kit::divider(ui);
            let mut on = phone.on;
            // Off while switching: a second press would undo the first.
            let changed = ui
                .add_enabled_ui(!crate::bt_music::busy(), |ui| {
                    kit::switch_row(
                        ui,
                        &mut on,
                        &t("Phone's sound on this PC (Bluetooth)"),
                        Some(&t(
                            "The phone plays its music and sound through this PC, as through a Bluetooth speaker. Calls are not affected.",
                        )),
                    )
                })
                .inner;
            if changed {
                self.set_bt_music(on);
            }
        }
    }

    /// One tip, with buttons for another one and to hide it until Broche opens again.
    fn tip_card(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.tip else { return };
        let p = pal(ui);
        kit::tinted_card(ui, p.secondary_container, |ui| {
            ui.horizontal(|ui| {
                kit::glyph(ui, "💡", 18.0, p.on_secondary_container);
                ui.label(RichText::new(t("Tip")).strong().color(p.on_secondary_container));
            });
            ui.add(egui::Label::new(RichText::new(t(TIPS[i])).color(p.on_secondary_container)).wrap());
            ui.horizontal_wrapped(|ui| {
                if kit::tonal_button(ui, &t("Another tip")).clicked() {
                    self.tip = Some((i + 1) % TIPS.len());
                }
                if kit::tonal_button(ui, &t("Hide")).clicked() {
                    self.tip = None;
                }
            });
        });
    }

    /// Follow the system, light or dark.
    pub(crate) fn theme_mode_choice(&mut self, ui: &mut egui::Ui) {
        let mut pref = self.theme_pref;
        if kit::segmented(
            ui,
            &mut pref,
            &[
                (ThemePreference::System, t("Follow the system")),
                (ThemePreference::Light, t("Light")),
                (ThemePreference::Dark, t("Dark")),
            ],
        ) {
            self.theme_pref = pref;
            let v = match pref {
                ThemePreference::Light => "light",
                ThemePreference::Dark => "dark",
                ThemePreference::System => "system",
            };
            Settings::update(|s| s.theme = v.into());
        }
    }

    /// Español or English.
    pub(crate) fn language_choice(&mut self, ui: &mut egui::Ui) {
        let mut lang = self.lang;
        if kit::segmented(ui, &mut lang, &[(Lang::Es, "Español".into()), (Lang::En, "English".into())]) {
            self.lang = lang;
            i18n::set(lang);
            Settings::update(|s| s.lang = if lang == Lang::Es { "es".into() } else { "en".into() });
        }
    }

    /// "Name of this PC": how phones show it. Saving an empty name goes back to the system's.
    pub(crate) fn device_name_field(&mut self, ui: &mut egui::Ui) {
        ui.label(t("Name of this PC"));
        let (_, save, enter) =
            kit::field_with_button(ui, &mut self.name_edit, &t("Name of this PC"), &t("Save"), false);
        supporting(ui, t("How your phone shows this PC. Useful when two PCs have the same name."));
        if save || enter {
            self.save_device_name();
        }
    }

    /// Saves what the name field holds, if it changed, and tells the phone.
    pub(crate) fn save_device_name(&mut self) {
        if broche_core::clean_device_name(&self.name_edit) != crate::device_name() {
            let chosen = broche_core::clean_device_name(&self.name_edit);
            Settings::update(|s| s.device_name = chosen.clone());
            let name = crate::device_name();
            self.name_edit = name.clone();
            if let Some(node) = self.node.clone() {
                let n = name.clone();
                self.rt.spawn(async move { node.set_device_name(&n).await });
            }
            let _ = self.tx.send(crate::Msg::Line(format!("{} {name}", t("This PC is now called"))));
        }
    }

    pub(crate) fn autostart_switch(&mut self, ui: &mut egui::Ui) {
        let mut auto = self.autostart_on;
        if kit::switch_row(
            ui,
            &mut auto,
            &t("Start with my session"),
            Some(&t("Broche opens minimized when you log in, so your phone connects right away.")),
        ) {
            self.set_autostart(auto);
        }
    }

    /// Turns starting with the session on or off. In a Flatpak the desktop is asked through
    /// the Background portal on the app's runtime, and a refusal turns the switch back.
    pub(crate) fn set_autostart(&mut self, on: bool) {
        if settings::packaging() == settings::Packaging::Flatpak {
            self.autostart_on = on;
            // The window repaints at least twice a second, so the answer shows without a nudge.
            let tx = self.tx.clone();
            self.rt.spawn(async move {
                if let Err(e) = settings::request_background(on).await {
                    let _ = tx.send(crate::Msg::AutostartFailed(on, e));
                }
            });
            return;
        }
        match settings::set_autostart(on) {
            Ok(()) => self.autostart_on = on,
            Err(e) => self.log.push(format!("{} {e}", t("Start with session:"))),
        }
    }

    pub(crate) fn clipboard_switch(&mut self, ui: &mut egui::Ui) {
        let mut on = self.clipboard_auto;
        if kit::switch_row(
            ui,
            &mut on,
            &t("Automatic clipboard"),
            Some(&t(
                "What you copy on this PC goes to the phone, and a notification tells you when the phone's arrives. When off, use Send text on Home. What you copy on the phone has its own switch there: Settings › Features › Clipboard.",
            )),
        ) {
            self.clipboard_auto = on;
            Settings::update(|s| s.clipboard_auto = on);
        }
    }

    /// What is set once: look, language, how the app runs and video.
    /// The user's own buttons on the phone's Deck, and the form to add one.
    fn deck_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            title(ui, &t("PC shortcuts on the phone"));
            supporting(
                ui,
                t(
                    "Your shortcuts show on the Custom page of PC shortcuts on the phone. What they do stays on this PC. The phone needs the permission to control the mouse and keyboard, and for commands also the one to run commands.",
                ),
            );
            let tiles = self.platform.deck.read().unwrap().clone();
            let mut remove = None;
            for (i, tile) in tiles.iter().enumerate() {
                kit::divider(ui);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&tile.label).strong());
                        supporting(
                            ui,
                            format!("{} · {}", deck::kind_label(&tile.action), tile.action.summary()),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if kit::danger_button(ui, &t("Delete")).clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                let mut tiles = tiles.clone();
                tiles.remove(i);
                self.deck_changed(tiles);
            }

            kit::divider(ui);
            ui.label(t("New button"));
            let ed = &mut self.deck_editor;
            kit::field(ui, &mut ed.name, &t("Button name"), 0.0);
            kit::segmented(
                ui,
                &mut ed.kind,
                &[
                    (deck::Kind::App, t("Open an app")),
                    (deck::Kind::Link, t("Open a link")),
                    (deck::Kind::Command, t("Run a command")),
                ],
            );
            match ed.kind {
                deck::Kind::App => {
                    let apps = ed.apps.get_or_insert_with(deck::installed_apps);
                    let chosen =
                        apps.iter().find(|(_, p)| Some(p) == ed.app.as_ref()).map(|(n, _)| n.clone());
                    let combo = egui::ComboBox::from_id_salt("deck_app")
                        .selected_text(chosen.unwrap_or_else(|| t("Choose an app")))
                        .width(280.0)
                        .show_ui(ui, |ui| {
                            for (name, path) in apps.iter() {
                                ui.selectable_value(&mut ed.app, Some(path.clone()), name);
                            }
                        });
                    kit::named(combo.response, &t("Choose an app"));
                }
                deck::Kind::Link => {
                    kit::named(kit::field(ui, &mut ed.value, "https://…", 0.0), &t("Open a link"));
                }
                deck::Kind::Command => {
                    kit::field(ui, &mut ed.value, &t("Program and arguments, e.g. notify-send Hello"), 0.0);
                    supporting(
                        ui,
                        t("It runs without a shell: pipes, redirections and variables are plain text."),
                    );
                    kit::switch(
                        ui,
                        &mut ed.confirmed,
                        &t("I understand the phone will be able to run this command on this PC"),
                    );
                }
            }
            let button = ed.button(&tiles);
            ui.add_enabled_ui(button.is_some(), |ui| {
                if kit::filled_button(ui, &t("Add button")).clicked()
                    && let Some(button) = button
                {
                    let mut tiles = tiles.clone();
                    tiles.push(button);
                    self.deck_editor =
                        deck::Editor { apps: self.deck_editor.apps.take(), ..Default::default() };
                    self.deck_changed(tiles);
                }
            });
        });
    }

    /// Saves the user's buttons and sends the new Deck to the phones.
    fn deck_changed(&mut self, tiles: Vec<broche_core::DeckTileConfig>) {
        if let Err(e) = deck::save(&tiles) {
            self.log.push(format!("{} {e}", t("Couldn't save the shortcuts:")));
        }
        let layout = deck::layout(&tiles);
        *self.platform.deck.write().unwrap() = tiles;
        if let Some(node) = self.node.clone() {
            let tx = self.tx.clone();
            self.rt.spawn(async move {
                if let Err(e) = node.set_deck_layout(layout).await {
                    let _ = tx.send(crate::Msg::Line(format!("{} {e}", t("Couldn't update the shortcuts:"))));
                }
            });
        }
    }

    /// The steps to hear calls on the PC, with what Broche can see of each. It only explains: pairing and
    /// the phone's permissions stay with the user.
    pub(crate) fn bt_calls_steps(&self, ui: &mut egui::Ui) {
        let paired = !crate::bt_calls::current().read().unwrap().is_empty();
        let (mark, state) = if !crate::bt_calls::available() {
            (
                "✖",
                t(
                    "This PC has no Bluetooth telephony (PipeWire with Bluetooth). Calls still show here, but their audio stays on the phone.",
                ),
            )
        } else if paired {
            ("✔", t("Ready: the phone is connected by Bluetooth."))
        } else {
            ("•", t("Waiting for the phone to be paired and connected (step 2)."))
        };
        ui.label(egui::RichText::new(format!("{mark} {state}")).strong());
        supporting(
            ui,
            t(
                "Call audio travels over Bluetooth, so the phone and this PC must be paired. Broche never changes your Bluetooth or phone settings: you do each step yourself.",
            ),
        );
        supporting(
            ui,
            t(
                "1. On the phone: Broche › Settings › Permissions › Calls, contacts and messages › Allow. Android asks for Phone, Call log and Contacts so Broche can show who is calling and answer or hang up for you.",
            ),
        );
        supporting(ui, t("2. On this PC: open the system Settings › Bluetooth and pair the phone."));
        supporting(
            ui,
            t(
                "3. On the phone, in that Bluetooth pairing, keep “Phone calls” on. You can turn off “Media audio” so music stays on the phone.",
            ),
        );
    }

    fn preferences_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 12.0;
        card(ui, |ui| {
            title(ui, &t("This PC"));
            self.device_name_field(ui);
        });
        card(ui, |ui| {
            title(ui, &t("Bluetooth calls"));
            let phones = crate::bt_calls::current().read().unwrap().clone();
            if !crate::bt_calls::available() {
                supporting(ui, t("Bluetooth calling is unavailable on this system."));
            } else if phones.is_empty() {
                supporting(ui, t("No Bluetooth phone connected."));
            } else {
                let name = |p: &crate::bt_calls::Gateway| {
                    if p.alias.is_empty() { p.address.clone() } else { p.alias.clone() }
                };
                if phones.len() == 1 {
                    ui.label(format!("{}: {}", t("Connected"), name(&phones[0])));
                } else {
                    ui.label(t("Phone for calls"));
                    let selected = phones
                        .iter()
                        .find(|p| p.address == self.bt_phone)
                        .map(name)
                        .unwrap_or_else(|| t("Choose a phone"));
                    let response =
                        egui::ComboBox::from_id_salt("bt_phone").selected_text(selected).show_ui(ui, |ui| {
                            for phone in &phones {
                                if ui
                                    .selectable_value(&mut self.bt_phone, phone.address.clone(), name(phone))
                                    .changed()
                                {
                                    Settings::update(|s| s.bt_phone = self.bt_phone.clone());
                                    crate::bt_calls::set_preferred(&self.bt_phone);
                                }
                            }
                        });
                    kit::named(response.response, &t("Phone for calls"));
                }
            }
            if kit::switch_row(ui, &mut self.bt_talk_on_answer, &t("Use PC audio when answering"), None) {
                Settings::update(|s| s.bt_talk_on_answer = self.bt_talk_on_answer);
            }
            self.bt_calls_steps(ui);
            supporting(
                ui,
                t("To return call audio to the phone, use the audio selector on the phone's call screen."),
            );
        });
        card(ui, |ui| {
            title(ui, &t("Appearance"));
            ui.label(t("Mode"));
            self.theme_mode_choice(ui);
            ui.add_space(4.0);
            ui.label(t("Colour scheme"));
            supporting(ui, t("Each scheme has a light and a dark version; the mode above picks one."));
            let (cols, w) = kit::columns_for(ui.available_width(), 220.0, 8.0, 3);
            for row in Scheme::ALL.chunks(cols) {
                ui.horizontal(|ui| {
                    for s in row {
                        if scheme_swatch(ui, *s, self.scheme == *s, w).clicked() {
                            self.scheme = *s;
                            let id = s.id().to_string();
                            Settings::update(|x| x.scheme = id);
                        }
                    }
                });
            }
            ui.add_space(4.0);
            ui.label(t("Text size"));
            let mut pct = (self.zoom * 100.0).round() as u32;
            if kit::segmented(
                ui,
                &mut pct,
                &[
                    (90, "90 %".into()),
                    (100, "100 %".into()),
                    (115, "115 %".into()),
                    (130, "130 %".into()),
                    (150, "150 %".into()),
                ],
            ) {
                self.zoom = pct as f32 / 100.0;
                self.zoom_set = false;
                Settings::update(|s| s.text_scale = pct);
            }
            supporting(ui, t("You can also use Ctrl + and Ctrl − anywhere."));
        });

        card(ui, |ui| {
            title(ui, &t("Window"));
            let mut tips = !self.hide_tips;
            if kit::switch_row(
                ui,
                &mut tips,
                &t("Tips on Home"),
                Some(&t("A short tip about something Broche can do, a different one each time it opens.")),
            ) {
                self.hide_tips = !tips;
                self.tip = tips.then(random_tip);
                Settings::update(|s| s.hide_tips = !tips);
            }
            let mut island = self.island_on;
            if kit::switch_row(
                ui,
                &mut island,
                &t("Island"),
                Some(&t(
                    "A pill at the top of the screen, above other windows, while there is a call, a transfer or music. It grows when the pointer is on it.",
                )),
            ) {
                self.island_on = island;
                Settings::update(|s| s.island = island);
            }
            if self.island_on {
                ui.label(t("Keep the island after the activity"));
                let options: Vec<(u32, String)> = [
                    (0, t("At once")),
                    (5, "5 s".into()),
                    (15, "15 s".into()),
                    (30, "30 s".into()),
                    (60, "1 min".into()),
                ]
                .into();
                if kit::segmented(ui, &mut self.island_linger, &options) {
                    let secs = self.island_linger;
                    Settings::update(|s| s.island_linger = secs);
                }
                self.island_place(ui);
            }
        });

        card(ui, |ui| {
            title(ui, &t("Language"));
            self.language_choice(ui);
        });

        self.deck_card(ui);

        card(ui, |ui| {
            title(ui, &t("Startup and closing"));
            self.autostart_switch(ui);
            kit::divider(ui);
            let mut keep = !self.close_quits;
            if kit::switch_row(
                ui,
                &mut keep,
                &t("Keep running when the window is closed"),
                Some(&t(
                    "Closing the window minimizes it and the phone stays connected. Quit from the tray icon or below.",
                )),
            ) {
                self.close_quits = !keep;
                let v = self.close_quits;
                Settings::update(|s| s.close_quits = v);
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if kit::tonal_button(ui, &t("Minimize")).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                if kit::danger_button(ui, &t("Quit application")).clicked() {
                    self.quitting = true;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if !self.tray_available.load(std::sync::atomic::Ordering::Relaxed) {
                supporting(ui, t("Your desktop has no system tray host."));
            }
        });

        card(ui, |ui| {
            title(ui, &t("Files"));
            ui.label(t("Files you receive are saved in:"));
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(received_dir().display().to_string())
                        .monospace()
                        .color(pal(ui).on_surface_variant),
                );
                if kit::tonal_button(ui, &t("Open folder")).clicked() {
                    let _ = std::process::Command::new("xdg-open").arg(received_dir()).spawn();
                }
            });
            kit::divider(ui);
            ui.label(t("Folder your phone can browse on this PC:"));
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&self.shared_edit).monospace().color(pal(ui).on_surface_variant));
                if kit::tonal_button(ui, &t("Choose folder…")).clicked() {
                    self.pick(Pick::SharedFolder);
                }
                if kit::tonal_button(ui, &t("Open folder")).clicked() {
                    let _ = std::process::Command::new("xdg-open").arg(&self.shared_edit).spawn();
                }
            });
            supporting(
                ui,
                t(
                    "The phone can only browse it if \"Browse files between these devices\" is on in Settings › Device.",
                ),
            );
        });

        card(ui, |ui| {
            title(ui, &t("Clipboard"));
            self.clipboard_switch(ui);
        });

        card(ui, |ui| {
            title(ui, &t("Notifications"));
            let mut on = self.phone_notify;
            if kit::switch_row(
                ui,
                &mut on,
                &t("Show the phone's notifications on this PC"),
                Some(&t(
                    "The ones that pop up on the phone also show as notifications of this PC; clicking one opens Broche. Silent and ongoing ones (music, navigation) stay on the phone.",
                )),
            ) {
                self.phone_notify = on;
                crate::phone_notify::set_enabled(on);
                Settings::update(|s| s.phone_notifications = on);
            }
        });

        card(ui, |ui| {
            title(ui, &t("Video quality"));
            supporting(
                ui,
                t(
                    "For the screens and the webcam. It is the same on the phone: change it on either. Applies to new streams.",
                ),
            );
            let mut high = self.experimental60;
            if kit::segmented(
                ui,
                &mut high,
                &[(false, t("Normal · 720p, 30 fps")), (true, t("High · 1080p, 60 fps (experimental)"))],
            ) {
                self.experimental60 = high;
                // Saved when the core answers (`Msg::VideoQuality`), with the time it chose.
                if let Some(node) = self.node.clone() {
                    let tx = self.tx.clone();
                    self.rt.spawn(async move {
                        let _ = tx.send(crate::Msg::VideoQuality(node.set_video_quality(high).await));
                    });
                }
            }
            supporting(
                ui,
                t(
                    "High is smoother but uses more battery and network. Go back to Normal if the picture stutters.",
                ),
            );
        });
    }

    /// What the selected phone may do on this PC, grouped and explained.
    fn access_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 12.0;
        self.doctor_card(ui);
        let Some(peer) = self.peer() else {
            self.no_phone(ui);
            return;
        };
        self.permissions_ui(ui, peer);
        card(ui, |ui| {
            title(ui, &t("This device"));
            supporting(ui, t("You will need to pair again to reconnect."));
            if kit::danger_button(ui, &t("Unpair this device")).clicked() {
                self.confirm_unpair = Some(peer);
            }
        });
    }

    fn about_tab(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                self.logo_image(ui, 56.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Broche").size(22.0).strong());
                    ui.label(format!("{} · Linux", env!("CARGO_PKG_VERSION")));
                });
            });
            ui.add_space(6.0);
            ui.label(t("Independent modified build based on NectarLink 0.1.4 by Bee/thtbee. It is not an official NectarLink release and is not endorsed by its author."));
            ui.label("GPL-3.0-or-later · MPL-2.0");
            supporting(ui, t("Original credits and licenses are kept."));
            supporting(ui, t("Free and open source. Your data stays on your devices."));
            ui.horizontal_wrapped(|ui| {
                if kit::tonal_button(ui, &t("Show the welcome again")).clicked() {
                    self.welcome_step = Some(0);
                }
                // Code, known problems and releases.
                if kit::tonal_button(ui, &t("Source code on GitHub")).clicked() {
                    let _ = std::process::Command::new("xdg-open")
                        .arg("https://github.com/RobertCJLA/broche")
                        .spawn();
                }
            });
        });

        card(ui, |ui| {
            title(ui, &t("Programs this app uses"));
            let missing = self.missing_programs.get_or_insert_with(deps::missing).clone();
            if missing.is_empty() {
                supporting(ui, t("Everything is installed."));
            } else {
                supporting(ui, t("Some features need programs that aren't installed:"));
                for need in &missing {
                    ui.label(format!("• {} ({})", t(need.feature), deps::programs(need)));
                }
                match deps::install_command(&missing) {
                    Some(command) => {
                        supporting(ui, t("Install them with:"));
                        ui.horizontal_wrapped(|ui| {
                            ui.add(egui::Label::new(RichText::new(&command).monospace()).selectable(true));
                            if kit::tonal_button(ui, &t("Copy")).clicked() {
                                ui.ctx().copy_text(command.clone());
                            }
                        });
                    }
                    None => supporting(ui, t("Install them with your distribution's package manager.")),
                }
            }
            if kit::tonal_button(ui, &t("Check again")).clicked() {
                self.missing_programs = None;
            }
        });

        card(ui, |ui| {
            egui::CollapsingHeader::new(t("Technical log"))
                .id_salt("technical-log")
                .show(ui, |ui| self.log_view(ui));
        });
    }
}

/// One entry of the navigation drawer or rail.
/// The phone on Home, drawn: a body with its screen lit in the accent while connected and a
/// battery ring in the middle (red when low, green while charging). Only a picture: the name,
/// state and battery are written next to it, so screen readers skip it.
/// Under "Connected": how the phone is reached, and which phone it is, in quieter text.
fn phone_facts(ui: &mut egui::Ui, dev: &Device, fg: egui::Color32) {
    let quiet = fg.gamma_multiply(0.8);
    if let LinkState::Online { path, rtt_ms } = &dev.link {
        let how = if *path == ConnectionPath::Relay { t("Through the internet") } else { t("Same network") };
        ui.label(RichText::new(format!("📶 {how} · {rtt_ms} ms")).size(13.0).color(quiet))
            .on_hover_text(t("Connection to the phone and its delay"));
    }
    if !dev.model.is_empty() {
        ui.label(RichText::new(format!("📱 {}", dev.model)).size(13.0).color(quiet))
            .on_hover_text(t("Model and system"));
    }
}

fn phone_hero(ui: &mut egui::Ui, online: bool, battery: Option<(u8, bool)>) {
    let p = pal(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(76.0, 140.0), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(18),
        p.surface_container_highest,
        Stroke::new(1.5_f32, if online { p.primary } else { p.outline }),
        egui::StrokeKind::Inside,
    );
    let screen = rect.shrink(5.0);
    let lit = if online { theme::mix(p.surface_container, p.primary, 0.28) } else { p.surface_container };
    painter.rect_filled(screen, CornerRadius::same(13), lit);
    painter.circle_filled(egui::pos2(screen.center().x, screen.top() + 8.0), 2.5, p.outline);
    let centre = screen.center();
    let (radius, width) = (24.0_f32, 5.0_f32);
    painter.circle_stroke(centre, radius, Stroke::new(width, p.outline_variant));
    if let Some((level, charging)) = battery {
        let colour = if charging {
            p.success
        } else if level <= 15 {
            p.error
        } else {
            p.primary
        };
        let turn = f32::from(level.min(100)) / 100.0 * std::f32::consts::TAU;
        let steps = 48;
        let points: Vec<egui::Pos2> = (0..=steps)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + turn * i as f32 / steps as f32;
                centre + radius * egui::vec2(a.cos(), a.sin())
            })
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(width, colour)));
        painter.text(
            centre,
            egui::Align2::CENTER_CENTER,
            format!("{level}%"),
            egui::FontId::proportional(13.0),
            p.on_surface,
        );
    }
}

/// Where a rail item's pill (the highlight behind its icon) goes.
fn nav_pill(item: egui::Rect) -> egui::Rect {
    egui::Rect::from_center_size(egui::pos2(item.center().x, item.top() + 18.0), egui::vec2(56.0, 32.0))
}

/// `count` > 0 shows a badge on the icon; the spoken name (`label`) says it too.
fn nav_item(ui: &mut egui::Ui, selected: bool, icon: &str, label: &str, count: usize) -> egui::Response {
    let p = pal(ui);
    let width = ui.available_width();
    let height = 60.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    let painter = ui.painter();
    let hot = resp.hovered() || resp.has_focus();
    let fg = if selected { p.on_secondary_container } else { p.on_surface_variant };
    let pill = nav_pill(rect);
    // The selected one's pill is drawn by the rail, so it can glide.
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), hot && !selected, theme::anim(0.12));
    if hover > 0.0 {
        painter.rect_filled(pill, CornerRadius::same(16), p.on_surface.gamma_multiply(0.08 * hover));
    }
    painter.text(
        pill.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(18.0),
        if selected { p.on_primary_container } else { fg },
    );
    if count > 0 {
        let centre = pill.right_top() + egui::vec2(-10.0, 4.0);
        painter.circle_filled(centre, 9.0, p.error);
        painter.text(
            centre,
            egui::Align2::CENTER_CENTER,
            if count > 99 { "99+".to_string() } else { count.to_string() },
            egui::FontId::proportional(10.5),
            // White or black, whichever reads better on this scheme's red.
            [egui::Color32::WHITE, egui::Color32::BLACK]
                .into_iter()
                .max_by(|a, b| theme::contrast(*a, p.error).total_cmp(&theme::contrast(*b, p.error)))
                .unwrap_or(egui::Color32::WHITE),
        );
    }
    let short: String = label.split_whitespace().next().unwrap_or(label).to_string();
    painter.text(
        egui::pos2(rect.center().x, rect.top() + 46.0),
        egui::Align2::CENTER_CENTER,
        short,
        egui::FontId::proportional(12.0),
        if selected { p.on_surface } else { p.on_surface_variant },
    );
    if resp.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            CornerRadius::same(RADIUS_FULL),
            Stroke::new(2.0_f32, p.primary),
            egui::StrokeKind::Inside,
        );
    }
    let name = label.to_string();
    let resp = resp.on_hover_text(label);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &name));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A Material tab: text with an indicator under it when selected.
fn tab_item(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    let p = pal(ui);
    let font = egui::FontId::proportional(15.0);
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font,
        if selected { p.primary } else { p.on_surface_variant },
    );
    let size = egui::vec2(galley.size().x + 32.0, 44.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    if resp.hovered() || resp.has_focus() {
        painter.rect_filled(rect, CornerRadius::same(8), p.on_surface.gamma_multiply(0.06));
    }
    painter.galley(rect.center() - galley.size() / 2.0, galley, p.on_surface);
    if selected {
        let bar = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 8.0, rect.bottom() - 3.0),
            egui::pos2(rect.right() - 8.0, rect.bottom()),
        );
        painter.rect_filled(bar, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, p.primary);
    }
    if resp.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            CornerRadius::same(8),
            Stroke::new(2.0_f32, p.primary),
            egui::StrokeKind::Inside,
        );
    }
    let name = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &name));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A card showing the colours of a scheme in the current mode.
fn scheme_swatch(ui: &mut egui::Ui, scheme: Scheme, selected: bool, width: f32) -> egui::Response {
    let cur = pal(ui);
    let s = theme::palette_for(scheme, ui.visuals().dark_mode);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 64.0), Sense::click());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(12),
        s.surface,
        Stroke::new(if selected { 3.0_f32 } else { 1.0 }, if selected { cur.primary } else { cur.outline }),
        egui::StrokeKind::Inside,
    );
    for (i, c) in [s.primary, s.primary_container, s.success, s.warning].iter().enumerate() {
        painter.circle_filled(egui::pos2(rect.left() + 20.0 + i as f32 * 20.0, rect.top() + 20.0), 8.0, *c);
    }
    let label = if selected { format!("✔ {}", scheme.label()) } else { scheme.label().to_string() };
    painter.text(
        egui::pos2(rect.left() + 12.0, rect.bottom() - 16.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        s.on_surface,
    );
    if resp.hovered() || resp.has_focus() {
        painter.rect_stroke(
            rect,
            CornerRadius::same(12),
            Stroke::new(2.0_f32, cur.primary),
            egui::StrokeKind::Inside,
        );
    }
    let name = scheme.label().to_string();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, &name));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl App {
    /// Where the island goes: a few places, back to the start, or locked where it is.
    fn island_place(&mut self, ui: &mut egui::Ui) {
        use crate::settings::IslandPlace;
        ui.label(t("Island position"));
        let mut chosen = None;
        ui.horizontal_wrapped(|ui| {
            for (place, name) in [
                (IslandPlace::TopLeft, t("Top left")),
                (IslandPlace::TopCentre, t("Top centre")),
                (IslandPlace::TopRight, t("Top right")),
                (IslandPlace::Bottom, t("Bottom")),
            ] {
                if kit::tonal_button(ui, &name).clicked() {
                    chosen = Some(place);
                }
            }
        });
        if let Some(place) = chosen {
            Settings::update(|s| s.island_at = place);
            if let Some(island) = &self.island {
                island.set_place(ui.ctx(), place);
            }
        }
        let mut locked = Settings::load().island_locked;
        if kit::switch_row(
            ui,
            &mut locked,
            &t("Lock the island's position"),
            Some(&t("Otherwise drag it by its background to put it anywhere on the screen.")),
        ) {
            Settings::update(|s| s.island_locked = locked);
            if let Some(island) = &self.island {
                island.locked.store(locked, std::sync::atomic::Ordering::Release);
            }
        }
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;

    #[test]
    fn search_ignores_accents_and_case_and_covers_every_card() {
        assert_eq!(fold("Compatibilidad ÚLTIMA Señal"), "compatibilidad ultima senal");
        let index = settings_index();
        // The nine preference cards, «This device» and the permission groups.
        assert_eq!(index.len(), 10 + crate::PERM_GROUPS.len());
        assert!(index.iter().any(|(title, tab, _)| *title == t("Bluetooth calls") && *tab == Tab::Perms));
        assert!(index.iter().any(|(_, tab, _)| *tab == Tab::Access));
    }
}
