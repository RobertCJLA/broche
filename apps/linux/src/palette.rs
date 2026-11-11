// SPDX-License-Identifier: GPL-3.0-or-later
//! The command palette (Ctrl+K), as in the original NectarLink: type a few letters of a page, a
//! setting or an action and press Enter. Arrow keys move, Escape closes. Matching ignores accents
//! and case, like the settings search.
use eframe::egui::{self, Key, Modifiers, RichText};

use crate::shell::{Section, fold, settings_index};
use crate::{App, Pick, Tab, i18n::t, kit, theme::pal};

/// Rows shown at once.
const SHOWN: usize = 8;

#[derive(Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    selected: usize,
    /// Give the search field the keyboard on the frame the palette opens.
    focus: bool,
}

#[derive(Clone)]
enum Act {
    Go(Tab),
    /// A settings card: its page, and its title to scroll to.
    Setting(Tab, String, Vec<String>),
    Ring,
    SendFiles,
    Pair,
    Check,
    Shelf,
    BtSound,
}

/// Other words people may type for an action, in both languages (like the settings search).
fn keywords(act: &Act) -> Vec<String> {
    let words: &[&str] = match act {
        Act::Ring => &["ring", "sonar", "timbre", "find", "buscar", "lost", "perdido"],
        Act::SendFiles => &["send", "enviar", "files", "archivos", "upload", "subir"],
        Act::Pair => &["pair", "vincular", "emparejar", "qr", "code", "código", "link"],
        Act::Shelf => &[
            "shelf",
            "estante",
            "panel",
            "photo",
            "foto",
            "screenshot",
            "captura",
            "received",
            "recibidos",
            "drag",
            "arrastrar",
        ],
        Act::BtSound => {
            &["bluetooth", "sound", "sonido", "music", "música", "audio", "speaker", "altavoz", "a2dp"]
        }
        Act::Check => {
            &["connection", "conexión", "doctor", "diagnóstico", "network", "red", "firewall", "cortafuegos"]
        }
        Act::Setting(_, _, words) => return words.clone(),
        Act::Go(_) => &[],
    };
    words.iter().map(|w| w.to_string()).collect()
}

impl App {
    /// Ctrl+K toggles the palette. Called before the page is drawn, so a focused text
    /// field never sees it (egui's text fields delete to the end of the line on Ctrl+K).
    pub(crate) fn palette_shortcut(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) {
            let was = self.palette.open;
            self.palette = CommandPalette { open: !was, focus: !was, ..Default::default() };
        }
    }

    pub(crate) fn command_palette(&mut self, ctx: &egui::Context) {
        if !self.palette.open {
            return;
        }
        let items = self.palette_items();
        // Keys first, so the text field doesn't take Enter and the arrows.
        let (down, up, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        let mut chosen = None;

        let shown = egui::Modal::new(egui::Id::new("command-palette")).show(ctx, |ui| {
            ui.set_width(460.0_f32.min(ctx.content_rect().width() - 48.0));
            let field =
                kit::search_field(ui, &mut self.palette.query, &t("Go to a page, a setting or an action…"));
            if std::mem::take(&mut self.palette.focus) {
                field.request_focus();
            }
            if field.changed() {
                self.palette.selected = 0;
            }
            // Filter after the field took this frame's typing, so Enter opens what is on screen.
            let query = fold(self.palette.query.trim());
            let matches: Vec<&(String, Act)> = items
                .iter()
                .filter(|(label, act)| {
                    query.is_empty()
                        || fold(label).contains(&query)
                        || keywords(act).iter().any(|w| fold(w).contains(&query))
                })
                .take(SHOWN)
                .collect();
            let last = matches.len().saturating_sub(1);
            if down {
                self.palette.selected = (self.palette.selected + 1).min(last);
            }
            if up {
                self.palette.selected = self.palette.selected.saturating_sub(1);
            }
            self.palette.selected = self.palette.selected.min(last);
            if enter {
                chosen = matches.get(self.palette.selected).map(|(_, a)| a.clone());
            }
            ui.add_space(6.0);
            if matches.is_empty() {
                kit::supporting(ui, t("Nothing matches."));
            }
            let p = pal(ui);
            for (i, (label, act)) in matches.iter().enumerate() {
                let row = ui.selectable_label(
                    i == self.palette.selected,
                    RichText::new(label).color(if i == self.palette.selected {
                        p.on_primary_container
                    } else {
                        p.on_surface
                    }),
                );
                if row.clicked() {
                    chosen = Some(act.clone());
                }
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(t("Arrow keys to move · Enter to open · Esc to close"))
                    .small()
                    .color(p.on_surface_variant),
            );
        });
        if shown.should_close() {
            self.palette.open = false;
        }
        if let Some(act) = chosen {
            self.palette.open = false;
            self.run_act(ctx, act);
        }
    }

    /// Everything the palette offers: actions on the phone first, then pages, then settings.
    fn palette_items(&self) -> Vec<(String, Act)> {
        let mut items = vec![];
        if let Some(dev) = self.selected_device().filter(|d| d.state.starts_with("online")) {
            let ring = if self.phone_ringing() { t("Stop ringing") } else { t("Find my phone") };
            items.push((format!("{ring} ({})", dev.name), Act::Ring));
            items.push((format!("{} ({})", t("Send files to the phone…"), dev.name), Act::SendFiles));
        }
        if let Some(phone) = crate::bt_music::current() {
            let label = if phone.on {
                t("Phone's sound back on the phone (Bluetooth)")
            } else {
                t("Phone's sound on this PC (Bluetooth)")
            };
            items.push((label, Act::BtSound));
        }
        items.push((t("Shelf"), Act::Shelf));
        items.push((t("Check connection"), Act::Check));
        items.push((t("Show pairing code"), Act::Pair));
        for (section, _, name) in Section::ALL {
            let pages = section.pages();
            if pages.is_empty() {
                items.push((t(name), Act::Go(section.first())));
            }
            for (tab, page) in pages {
                items.push((format!("{} › {}", t(name), t(page)), Act::Go(*tab)));
            }
        }
        for (title, tab, words) in settings_index() {
            items.push((format!("{} › {title}", t("Settings")), Act::Setting(tab, title, words)));
        }
        items
    }

    fn run_act(&mut self, ctx: &egui::Context, act: Act) {
        match act {
            Act::Go(tab) => self.go(tab),
            Act::Setting(tab, title, _) => {
                self.go(tab);
                ctx.data_mut(|d| d.insert_temp(egui::Id::new(kit::SCROLL_TO), title));
            }
            Act::Ring => {
                if let Some(peer) = self.peer() {
                    self.toggle_ring(peer);
                }
            }
            Act::SendFiles => {
                if let Some(peer) = self.peer() {
                    self.pick(Pick::Send(peer));
                }
            }
            Act::Pair => {
                self.go(Tab::Pair);
                // A pairing already running keeps going; a second one would orphan it.
                if self.pair_task.is_none() {
                    self.start_pairing();
                }
            }
            Act::Check => {
                self.go(Tab::Access);
                self.run_doctor(ctx);
            }
            Act::Shelf => self.toggle_shelf(ctx),
            Act::BtSound => {
                if let Some(phone) = crate::bt_music::current() {
                    self.set_bt_music(!phone.on);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ringing_is_found_by_the_words_people_type() {
        let found = |q: &str| keywords(&Act::Ring).iter().any(|w| fold(w).contains(&fold(q)));
        assert!(found("son"));
        assert!(found("Ring"));
        assert!(!found("firewall"));
        assert!(keywords(&Act::Check).iter().any(|w| fold(w).contains(&fold("conexion"))));
    }
}
