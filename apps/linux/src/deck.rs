// SPDX-License-Identifier: GPL-3.0-or-later
//! The phone's Deck for this PC: the built-in page plus the user's own buttons (open an app,
//! open a link, run a command), kept in `deck.json` next to the settings. What a button does
//! stays on this PC; the phone only learns its name and kind (`docs/protocol/deck.md`).

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use broche_core::{DeckAction, DeckConfig, DeckLayout, DeckPageConfig, DeckTileConfig, deck_kinds};

use crate::i18n::t;

const CUSTOM_PAGE: &str = "custom";

/// The built-in buttons this PC can run (see `LinuxPlatform::deck_press`).
const BUILT_IN: [&str; 7] =
    ["play_pause", "prev_track", "next_track", "vol_down", "vol_up", "vol_mute", "lock_pc"];

/// The user's buttons, saved in `deck.json`. A missing or broken file means none.
pub fn load() -> Vec<DeckTileConfig> {
    let Some(path) = crate::settings::deck_path().filter(|p| p.exists()) else { return Vec::new() };
    DeckConfig::load(&path)
        .pages
        .into_iter()
        .find(|p| p.id == CUSTOM_PAGE)
        .map(|p| p.tiles)
        .unwrap_or_default()
}

pub fn save(tiles: &[DeckTileConfig]) -> Result<(), String> {
    let path = crate::settings::deck_path().ok_or("no config folder")?;
    let page = DeckPageConfig { id: CUSTOM_PAGE.into(), name: "Custom".into(), tiles: tiles.to_vec() };
    DeckConfig { pages: vec![page] }.save(&path).map_err(|e| e.to_string())
}

/// A new button with the kind's icon and colour, or `None` when the action isn't valid.
pub fn tile(existing: &[DeckTileConfig], label: &str, action: DeckAction) -> Option<DeckTileConfig> {
    let id = (1..).map(|n| format!("custom_{n}")).find(|id| existing.iter().all(|t| &t.id != id))?;
    let kind = action.kind();
    let tile = DeckTileConfig {
        id,
        label: label.trim().to_owned(),
        icon: deck_kinds::default_icon(kind).into(),
        color: deck_kinds::default_color(kind).into(),
        action,
    };
    tile.sanitized()
}

/// What a new button does, in the editor.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    App,
    Link,
    Command,
}

/// The "new button" form in Preferences.
#[derive(Default)]
pub struct Editor {
    pub name: String,
    pub kind: Kind,
    /// The link or the command.
    pub value: String,
    pub app: Option<PathBuf>,
    /// For a command: the user confirmed the phone may run it.
    pub confirmed: bool,
    /// Read when the form first needs them.
    pub apps: Option<Vec<(String, PathBuf)>>,
}

impl Editor {
    /// The button the form describes, if it is complete and valid.
    pub fn button(&self, existing: &[DeckTileConfig]) -> Option<DeckTileConfig> {
        let action = match self.kind {
            Kind::App => DeckAction::LaunchApp { path: self.app.as_ref()?.to_string_lossy().into_owned() },
            Kind::Link => DeckAction::OpenUrl { url: self.value.trim().to_owned() },
            Kind::Command if self.confirmed => {
                DeckAction::RunCommand { command: self.value.trim().to_owned() }
            }
            Kind::Command => return None,
        };
        (existing.len() < broche_core::MAX_DECK_TILES_PER_PAGE).then(|| tile(existing, &self.name, action))?
    }
}

/// A short name for what a saved button does.
pub fn kind_label(action: &DeckAction) -> String {
    match action {
        DeckAction::LaunchApp { .. } => t("Open an app"),
        DeckAction::OpenUrl { .. } => t("Open a link"),
        DeckAction::RunCommand { .. } => t("Run a command"),
        _ => String::new(),
    }
}

/// What the phone shows: the built-in page, then "Custom" when there are own buttons.
pub fn layout(custom: &[DeckTileConfig]) -> DeckLayout {
    let mut deck = DeckConfig::default_deck().to_wire_layout();
    for page in &mut deck.pages {
        page.tiles.retain(|t| BUILT_IN.contains(&t.id.as_str()));
        for tile in &mut page.tiles {
            tile.label = t(&tile.label); // the phone shows these names as they are
        }
    }
    if !custom.is_empty() {
        let page = DeckPageConfig { id: CUSTOM_PAGE.into(), name: t("Custom"), tiles: custom.to_vec() };
        deck.pages.push(page.to_wire());
    }
    deck
}

/// Runs one of the user's buttons. The core already checked the permissions (`remote_input`,
/// and `commands` for a command) and the action was validated when it was saved and loaded.
pub fn run(action: &DeckAction) -> Result<(), String> {
    match action {
        DeckAction::OpenUrl { url } => spawn(Command::new("xdg-open").arg(url)),
        // Apps and commands are the system's: in the Flatpak they run outside the sandbox.
        DeckAction::LaunchApp { path } => spawn(crate::platform::host("gio").arg("launch").arg(path)),
        DeckAction::RunCommand { command } => {
            let args = broche_core::command_args(command).ok_or("invalid command")?;
            spawn(crate::platform::host(&args[0]).args(&args[1..]))
        }
        _ => Err("This action is unavailable on Linux".into()),
    }
}

/// Starts `cmd` without waiting for it, and reaps it when it ends.
fn spawn(cmd: &mut Command) -> Result<(), String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Apps in the menu (`.desktop` files that aren't hidden), by name, for the button editor.
/// In the Flatpak the sandbox only holds Broche's own: the system lists them instead.
pub fn installed_apps() -> Vec<(String, PathBuf)> {
    if crate::settings::packaging() == crate::settings::Packaging::Flatpak {
        return host_apps();
    }
    let mut dirs: Vec<PathBuf> = dirs::data_dir().into_iter().collect();
    let system = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(std::env::split_paths(&system));
    dirs.push("/var/lib/flatpak/exports/share".into());
    dirs.extend(dirs::data_dir().map(|d| d.join("flatpak/exports/share")));
    let mut apps: Vec<(String, PathBuf)> = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir.join("applications")) else { continue };
        for path in entries.flatten().map(|e| e.path()) {
            let id = path.file_name().map(|n| n.to_owned());
            if apps.iter().any(|(_, p)| p.file_name().map(|n| n.to_owned()) == id) {
                continue; // the first folder wins, as in the menu
            }
            if let Some(name) = app_name(&path) {
                apps.push((name, path));
            }
        }
    }
    apps.sort_by_key(|(name, _)| name.to_lowercase());
    apps
}

/// The system's menu entries, asked outside the sandbox: each file's path after a record
/// separator, then just the lines that say whether and how it is shown.
fn host_apps() -> Vec<(String, PathBuf)> {
    const LIST: &str = r#"for d in "$HOME/.local/share" /usr/local/share /usr/share \
        /var/lib/flatpak/exports/share "$HOME/.local/share/flatpak/exports/share"; do
        for f in "$d"/applications/*.desktop; do
            [ -f "$f" ] || continue
            printf '\036%s\n' "$f"
            grep -E '^(\[|Type=|Name=|NoDisplay=|Hidden=)' "$f"
        done
    done"#;
    let Ok(out) = crate::platform::host("sh").args(["-c", LIST]).output() else { return Vec::new() };
    parse_listing(&String::from_utf8_lossy(&out.stdout))
}

/// Apps from `host_apps`' listing; the first folder wins, as in the menu.
fn parse_listing(listing: &str) -> Vec<(String, PathBuf)> {
    let mut apps: Vec<(String, PathBuf)> = Vec::new();
    for record in listing.split('\u{1e}') {
        let Some((path, text)) = record.split_once('\n') else { continue };
        let path = PathBuf::from(path);
        let id = path.file_name().map(|n| n.to_owned());
        if apps.iter().any(|(_, p)| p.file_name().map(|n| n.to_owned()) == id) {
            continue;
        }
        if let Some(name) = app_name_in(&path, text) {
            apps.push((name, path));
        }
    }
    apps.sort_by_key(|(name, _)| name.to_lowercase());
    apps
}

/// The `Name=` of a menu entry, or `None` if it isn't a visible application.
fn app_name(path: &Path) -> Option<String> {
    app_name_in(path, &std::fs::read_to_string(path).ok()?)
}

fn app_name_in(path: &Path, text: &str) -> Option<String> {
    if path.extension()? != "desktop" {
        return None;
    }
    let entry = text.split("\n[").next()?; // only the [Desktop Entry] group
    let value = |key: &str| entry.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=')).map(str::trim);
    let shown = value("Type") == Some("Application")
        && value("NoDisplay") != Some("true")
        && value("Hidden") != Some("true");
    shown.then(|| value("Name").map(str::to_owned)).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_visible_apps_have_a_name() {
        let dir = std::env::temp_dir().join(format!("broche-deck-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let entry = |name: &str, body: &str| {
            let p = dir.join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        let files = entry("files.desktop", "[Desktop Entry]\nType=Application\nName=Files\nExec=nautilus\n");
        let hidden = entry("hidden.desktop", "[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\n");
        let link = entry("link.desktop", "[Desktop Entry]\nType=Link\nName=Site\n");
        let action = entry(
            "act.desktop",
            "[Desktop Entry]\nType=Application\nName=Main\n[Desktop Action new]\nName=Other\n",
        );
        assert_eq!(app_name(&files).as_deref(), Some("Files"));
        let listing = "\u{1e}/home/a/.local/share/applications/files.desktop\n[Desktop Entry]\nType=Application\nName=Mine\n\
            \u{1e}/usr/share/applications/files.desktop\n[Desktop Entry]\nType=Application\nName=Files\n\
            \u{1e}/usr/share/applications/hid.desktop\n[Desktop Entry]\nType=Application\nName=H\nNoDisplay=true\n\
            \u{1e}/usr/share/applications/a.desktop\n[Desktop Entry]\nType=Application\nName=Atlas\n";
        let names: Vec<String> = parse_listing(listing).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["Atlas", "Mine"]); // the user's own wins; hidden ones are left out
        assert_eq!(app_name(&hidden), None);
        assert_eq!(app_name(&link), None);
        assert_eq!(app_name(&action).as_deref(), Some("Main"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn new_buttons_get_free_ids_and_invalid_actions_are_refused() {
        let first = tile(&[], "Site", DeckAction::OpenUrl { url: "https://example.com".into() }).unwrap();
        assert_eq!(first.id, "custom_1");
        let second =
            tile(std::slice::from_ref(&first), "Run", DeckAction::RunCommand { command: "true".into() });
        assert_eq!(second.unwrap().id, "custom_2");
        assert!(tile(&[], "Bad", DeckAction::OpenUrl { url: "file:///etc/passwd".into() }).is_none());
        assert!(tile(&[], "Bad", DeckAction::RunCommand { command: "echo 'open".into() }).is_none());
    }

    #[test]
    fn the_custom_page_follows_the_built_in_one() {
        let site = tile(&[], "Site", DeckAction::OpenUrl { url: "https://example.com".into() }).unwrap();
        assert_eq!(layout(&[]).pages.len(), 1);
        let deck = layout(&[site]);
        assert_eq!(deck.pages.len(), 2);
        assert_eq!(deck.pages[1].tiles[0].kind, deck_kinds::OPEN_URL);
        assert!(deck.is_valid());
    }
}
