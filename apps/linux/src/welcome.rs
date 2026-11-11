// SPDX-License-Identifier: GPL-3.0-or-later
//! Welcome and first setup: language and look, linking the phone, what it may do, how Broche
//! runs, and a last page on where things are. Shown on the first start (until finished or
//! skipped) and again from Settings › About.
use crate::{App, i18n::t, kit, settings::Settings, theme::pal};
use eframe::egui::{self, RichText};

/// How many pages the welcome has.
pub(crate) const STEPS: u8 = 5;

impl App {
    /// Draws the welcome instead of the app while it is open.
    pub(crate) fn welcome(&mut self, ctx: &egui::Context) {
        let Some(step) = self.welcome_step else { return };
        self.apply_look(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(24).fill(ctx.style().visuals.panel_fill))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    // A column of at most 620 px in the middle of the window, its contents left-aligned.
                    let width = ui.available_width().min(620.0);
                    let side = (ui.available_width() - width) / 2.0;
                    ui.horizontal_top(|ui| {
                        ui.add_space(side);
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            ui.spacing_mut().item_spacing.y = 12.0;
                            ui.horizontal(|ui| {
                                self.logo_image(ui, 40.0);
                                ui.label(RichText::new("Broche").size(22.0).strong());
                            });
                            kit::supporting(ui, format!("{} {} {} {}", t("Step"), step + 1, t("of"), STEPS));
                            let fill = pal(ui).primary;
                            ui.add(
                                egui::ProgressBar::new(f32::from(step + 1) / f32::from(STEPS))
                                    .desired_height(6.0)
                                    .fill(fill),
                            );
                            match step {
                                0 => self.welcome_start(ui),
                                1 => self.welcome_link(ui),
                                2 => self.welcome_permissions(ui),
                                3 => self.welcome_habits(ui),
                                _ => self.welcome_done(ui),
                            }
                            ui.add_space(8.0);
                            self.welcome_buttons(ui, step);
                        });
                    });
                });
            });
    }

    fn welcome_start(&mut self, ui: &mut egui::Ui) {
        kit::card(ui, |ui| {
            ui.label(RichText::new(t("Welcome to Broche")).size(24.0).strong());
            kit::supporting(
                ui,
                t(
                    "Your Android phone and this PC, together: files, messages, notifications, its screen and camera, over your own network and end-to-end encrypted.",
                ),
            );
        });
        kit::card(ui, |ui| {
            kit::title(ui, &t("Language"));
            self.language_choice(ui);
            kit::title(ui, &t("Mode"));
            self.theme_mode_choice(ui);
        });
        kit::card(ui, |ui| self.device_name_field(ui));
    }

    fn welcome_link(&mut self, ui: &mut egui::Ui) {
        if let Some(dev) = self.selected_device() {
            kit::card(ui, |ui| {
                kit::title(ui, &t("Your phone"));
                ui.label(format!("✔ {} · {}", dev.name, t("already linked.")));
                kit::supporting(ui, t("To link another phone later, go to Settings › Pair."));
            });
            return;
        }
        kit::card(ui, |ui| {
            kit::title(ui, &t("Link your phone"));
            for line in [
                "1. Install Broche on your Android phone.",
                "2. Open it and choose to link a PC.",
                "3. Scan the code below and check that both show the same numbers.",
            ] {
                ui.label(t(line));
            }
            kit::supporting(ui, t("Both need to be on the same Wi-Fi or network."));
        });
        self.pairing_card(ui);
    }

    fn welcome_permissions(&mut self, ui: &mut egui::Ui) {
        // Pairing asks this on the Link page; here the welcome covers it.
        self.profile_prompt = None;
        let Some(peer) = self.peer() else {
            kit::card(ui, |ui| {
                kit::title(ui, &t("What may the phone do?"));
                kit::supporting(ui, t("Link a phone first. You can choose this later in Settings › Device."));
            });
            return;
        };
        let current = self.cached_toggles(peer).and_then(|l| broche_core::features::profile_of(&l));
        self.profile_card(ui, peer, current);
        kit::card(ui, |ui| {
            kit::title(ui, &t("Hear and answer calls on this PC"));
            self.bt_calls_steps(ui);
        });
    }

    fn welcome_habits(&mut self, ui: &mut egui::Ui) {
        kit::card(ui, |ui| {
            kit::title(ui, &t("Everyday use"));
            self.autostart_switch(ui);
            kit::divider(ui);
            self.clipboard_switch(ui);
        });
    }

    fn welcome_done(&mut self, ui: &mut egui::Ui) {
        kit::card(ui, |ui| {
            ui.label(RichText::new(t("All set")).size(24.0).strong());
            for line in [
                "Home has what you use every day: find your phone, send files, its volume and music.",
                "The tray icon keeps Broche at hand: the phone's screen, files and folders.",
                "Everything you chose here can be changed in Settings.",
            ] {
                ui.label(t(line));
            }
        });
    }

    fn welcome_buttons(&mut self, ui: &mut egui::Ui, step: u8) {
        ui.horizontal(|ui| {
            if step > 0 && kit::tonal_button(ui, &t("Back")).clicked() {
                self.welcome_step = Some(step - 1);
            }
            let last = step + 1 >= STEPS;
            if !last && kit::tonal_button(ui, &t("Skip the welcome")).clicked() {
                self.finish_welcome();
            }
            let next = if last { t("Start using Broche") } else { t("Next") };
            if kit::filled_button(ui, &next).clicked() {
                if step == 0 {
                    // The name is kept even if the field's own Save wasn't pressed.
                    self.save_device_name();
                }
                if last {
                    self.finish_welcome();
                } else {
                    self.welcome_step = Some(step + 1);
                }
            }
        });
    }

    fn finish_welcome(&mut self) {
        self.welcome_step = None;
        Settings::update(|s| s.welcome_done = true);
    }
}
