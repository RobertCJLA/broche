// SPDX-License-Identifier: GPL-3.0-or-later
//! History: what moved between this PC and its phones (files, clips, links, photos, screen
//! sessions), newest first, searchable and filtered by kind. The core keeps it
//! (`Node::timeline_page`); this page only shows it, as the original NectarLink's Timeline did.
use broche_core::{TimelineEntry, TimelineKind, TimelinePage, TimelineQuery};
use eframe::egui::{self, RichText};

use crate::{App, i18n::t, kit, theme::pal};

/// Rows asked for at a time; "Show more" adds this many.
const PAGE: u32 = 50;

/// What the History page remembers between frames.
#[derive(Default)]
pub struct History {
    pub search: String,
    pub kind: Option<TimelineKind>,
    pub limit: u32,
    pub page: Option<TimelinePage>,
    /// Ask the core again (opened, filter changed, or the core said the history changed).
    pub stale: bool,
    /// What `page` was asked with, to notice a new search.
    asked: Option<(String, Option<TimelineKind>, u32)>,
}

impl App {
    pub(crate) fn history_tab(&mut self, ui: &mut egui::Ui) {
        let Some(node) = self.node.clone() else { return };
        ui.horizontal_wrapped(|ui| {
            let kinds = [
                (None, t("All")),
                (Some(TimelineKind::File), t("Files")),
                (Some(TimelineKind::Clip), t("Clips")),
                (Some(TimelineKind::Link), t("Links")),
                (Some(TimelineKind::Photo), t("Photos")),
                (Some(TimelineKind::Session), t("Screen sessions")),
            ];
            for (kind, label) in kinds {
                if kit::chip(ui, self.history.kind == kind, &label).clicked() {
                    self.history.kind = kind;
                    self.history.limit = PAGE;
                }
            }
        });
        ui.add_space(4.0);
        kit::search_field(ui, &mut self.history.search, &t("Search history"));
        ui.add_space(8.0);

        let h = &mut self.history;
        h.limit = h.limit.max(PAGE);
        let wanted = (h.search.trim().to_string(), h.kind, h.limit);
        if h.stale || h.asked.as_ref() != Some(&wanted) {
            let query = TimelineQuery {
                kind: h.kind,
                device: None,
                search: Some(wanted.0.clone()).filter(|s| !s.is_empty()),
                offset: 0,
                limit: h.limit,
            };
            h.page = node.timeline_page(&query).ok();
            h.asked = Some(wanted);
            h.stale = false;
        }
        let Some(page) = self.history.page.clone() else {
            kit::card(ui, |ui| kit::supporting(ui, t("History couldn't be read.")));
            return;
        };
        if page.entries.is_empty() {
            kit::card(ui, |ui| {
                kit::empty_state(
                    ui,
                    "🕘",
                    &t("Nothing in your history yet."),
                    &t("Files, clips, links, photos and screen sessions with your phone show up here."),
                );
            });
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            kit::card(ui, |ui| {
                for (i, e) in page.entries.iter().enumerate() {
                    if i > 0 {
                        kit::divider(ui);
                    }
                    history_row(ui, e);
                }
            });
            if page.has_more {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if kit::tonal_button(ui, &t("Show more")).clicked() {
                        self.history.limit += PAGE;
                    }
                });
            }
        });
    }
}

/// The core writes some titles in English ("Files", "a.txt (+2 more)"); file names and the
/// like stay as they are.
fn core_title(title: &str) -> String {
    match title {
        "Files" => t("Files"),
        "Copied text" | "Text clip" => t("Copied text"),
        "Image clip" => t("Copied image"),
        "Map location" => t("Map location"),
        "Handoff document" => t("Shared document"),
        "Phone webcam" => t("Phone webcam"),
        _ => {
            if let Some(place) = title.strip_suffix(" on a map") {
                return format!("{place} {}", t("on a map"));
            }
            if let Some((name, n)) = title.strip_suffix(" more)").and_then(|s| s.rsplit_once(" (+"))
                && n.parse::<u32>().is_ok()
            {
                return format!("{name} (+{n} {})", t("more"));
            }
            title.to_owned()
        }
    }
}

fn history_row(ui: &mut egui::Ui, e: &TimelineEntry) {
    let p = pal(ui);
    let icon = match e.kind {
        TimelineKind::File => "📄",
        TimelineKind::Clip => "📋",
        TimelineKind::Link => "🔗",
        TimelineKind::Photo => "🖼",
        TimelineKind::Session => "📺",
    };
    let title = if e.title.trim().is_empty() { t("(no title)") } else { core_title(&e.title) };
    let mut about = vec![
        format!("{} {}", if e.incoming { t("Received from") } else { t("Sent to") }, e.device_name),
        ago(chrono_now() - e.timestamp),
    ];
    if e.size_bytes > 0 {
        about.push(crate::human_size(e.size_bytes));
    }
    if let Some(detail) = shown_detail(e) {
        about.push(detail);
    }
    ui.horizontal(|ui| {
        kit::glyph(ui, icon, 20.0, p.on_surface_variant);
        ui.vertical(|ui| {
            ui.add(egui::Label::new(RichText::new(&title).color(p.on_surface)).truncate());
            ui.label(RichText::new(about.join(" · ")).small().color(p.on_surface_variant));
        });
        // Open what can be opened here: a link, or a file that is on this PC.
        let local = e.target.lines().next().map(std::path::Path::new).filter(|p| p.exists());
        let opens = match e.kind {
            TimelineKind::Link => Some(e.target.clone()).filter(|u| u.starts_with("http")),
            _ => local.map(|p| p.display().to_string()),
        };
        if let Some(target) = opens {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::tonal_button(ui, &t("Open")).clicked() {
                    let _ = std::process::Command::new("xdg-open").arg(target).spawn();
                }
            });
        }
    });
}

/// The core's detail, minus what says nothing ("text", "File", the link again) and with its one
/// English word that matters translated.
fn shown_detail(e: &TimelineEntry) -> Option<String> {
    let d = e.detail.trim();
    match d {
        "" | "text" | "File" => None,
        "image" => Some(t("Image")),
        _ if d == e.target.trim() => None,
        _ => Some(d.to_string()),
    }
}

pub(crate) fn chrono_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// "5 min ago", "3 h ago", "2 days ago", in the app's language.
pub(crate) fn ago(secs: i64) -> String {
    let secs = secs.max(0);
    match secs {
        0..60 => t("Just now"),
        60..3600 => t("{} min ago").replace("{}", &(secs / 60).to_string()),
        3600..86_400 => t("{} h ago").replace("{}", &(secs / 3600).to_string()),
        86_400..172_800 => t("Yesterday"),
        _ => t("{} days ago").replace("{}", &(secs / 86_400).to_string()),
    }
}

/// How long a call lasted: "45 s", "4:14", or nothing for one that wasn't answered.
pub(crate) fn call_length(secs: u32) -> Option<String> {
    match secs {
        0 => None,
        1..60 => Some(format!("{secs} s")),
        _ => Some(format!("{}:{:02}", secs / 60, secs % 60)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_lengths_read_as_minutes_and_seconds() {
        assert_eq!(call_length(0), None);
        assert_eq!(call_length(45).as_deref(), Some("45 s"));
        assert_eq!(call_length(254).as_deref(), Some("4:14"));
        assert_eq!(call_length(3725).as_deref(), Some("62:05"));
    }

    #[test]
    fn times_read_naturally() {
        assert_eq!(ago(30), t("Just now"));
        assert!(ago(5 * 60).starts_with('5') || ago(5 * 60).contains('5'));
        assert!(ago(3 * 3600).contains('3'));
        assert!(ago(2 * 86_400).contains('2'));
        assert_eq!(ago(30 * 3600), t("Yesterday"), "one day is not \"1 days ago\"");
    }
}

#[cfg(test)]
mod title_tests {
    use super::core_title;

    #[test]
    fn core_titles_are_translated_and_names_kept() {
        use crate::i18n::t;
        // Whatever language another test left set: only the fixed words change.
        assert_eq!(core_title("a.txt (+2 more)"), format!("a.txt (+2 {})", t("more")));
        assert_eq!(core_title("Files"), t("Files"));
        assert_eq!(core_title("informe (+x more).pdf"), "informe (+x more).pdf");
        assert_eq!(core_title("notas.txt"), "notas.txt");
    }
}
