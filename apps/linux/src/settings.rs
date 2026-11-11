// SPDX-License-Identifier: GPL-3.0-or-later
//! Preferences that outlive the window: language, theme, the folder phones may browse,
//! and the desktop portal's restore token (so GNOME doesn't ask again every time).
//! Stored as `key=value` lines in `~/.config/broche/settings.conf`.

use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// `es` or `en`; empty: follow the system.
    pub lang: String,
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// The folder phones may browse; empty: the default.
    pub shared_dir: String,
    /// Restore token from the RemoteDesktop portal.
    pub portal_token: String,
    /// High video quality (1080p, 60 fps), kept the same on the phone: see `video_at`.
    pub experimental60: bool,
    /// When the video quality was chosen (Unix ms), so the newer choice wins on both devices.
    pub video_at: i64,
    /// Colour scheme id (see `theme::Scheme`); empty: Catppuccin Mocha.
    pub scheme: String,
    /// Text size in percent; 0: 100 %.
    pub text_scale: u32,
    /// Closing the window quits the app instead of keeping it running.
    pub close_quits: bool,
    /// Phone storage shown as a list instead of a grid.
    pub storage_list: bool,
    /// Compact window: icon rail always and tighter spacing.
    /// Flatpak only: autostart was requested from the Background portal.
    pub flatpak_autostart: bool,
    /// What is copied on this PC goes to the phone, with a notification when the phone's arrives.
    pub clipboard_auto: bool,
    /// No tip card on Home.
    pub hide_tips: bool,
    /// The island: calls, transfers and music in a pill at the top of the screen.
    pub island: bool,
    /// Seconds the island stays after its activity stops.
    pub island_linger: u32,
    /// Where the island goes (Preferences › Window, or where it was dragged).
    pub island_at: IslandPlace,
    /// The island can't be dragged.
    pub island_locked: bool,
    /// The phone's notifications show on the desktop too (on unless switched off).
    pub phone_notifications: bool,
    /// The welcome was finished or skipped.
    pub welcome_done: bool,
    /// How phones show this PC; empty: the system's name (`main::default_device_name`).
    pub device_name: String,
    /// Preferred Bluetooth phone address when several gateways are connected.
    pub bt_phone: String,
    /// Answering on this PC also activates its Bluetooth call audio.
    pub bt_talk_on_answer: bool,
}

fn path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("BROCHE_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("settings.conf"));
    }
    dirs::config_dir().map(|d| d.join("broche").join("settings.conf"))
}

/// Where the user's own Deck buttons are saved (`deck.json`, next to the settings).
pub fn deck_path() -> Option<PathBuf> {
    path().map(|p| p.with_file_name("deck.json"))
}

/// No file yet is the same as an empty one (some preferences are on unless switched off).
impl Default for Settings {
    fn default() -> Self {
        Self::parse("")
    }
}

impl Settings {
    /// The longest side of the phone's picture: High quality is 1080p, like on the phone.
    pub fn video_max_size(&self) -> u32 {
        if self.experimental60 { 1920 } else { 1280 }
    }

    /// More pixels need more bits, or the picture turns to blocks when it moves.
    pub fn video_bitrate(&self) -> u32 {
        if self.experimental60 { 12_000_000 } else { 6_000_000 }
    }

    pub fn video_fps(&self) -> u32 {
        if self.experimental60 { 60 } else { 30 }
    }

    /// Text size as a zoom factor, limited to sensible values.
    pub fn zoom(&self) -> f32 {
        if self.text_scale == 0 { 1.0 } else { (self.text_scale.clamp(80, 160) as f32) / 100.0 }
    }

    pub fn parse(text: &str) -> Self {
        let map: BTreeMap<&str, &str> =
            text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
        let get = |k: &str| map.get(k).map(|v| v.to_string()).unwrap_or_default();
        Self {
            lang: get("lang"),
            theme: get("theme"),
            shared_dir: get("shared_dir"),
            portal_token: get("portal_token"),
            experimental60: get("experimental60") == "true",
            video_at: get("video_at").parse().unwrap_or(0),
            scheme: get("scheme"),
            text_scale: get("text_scale").parse().unwrap_or(0),
            close_quits: get("close_quits") == "true",
            storage_list: get("storage_list") == "true",
            flatpak_autostart: get("flatpak_autostart") == "true",
            clipboard_auto: get("clipboard_auto") == "true",
            hide_tips: get("hide_tips") == "true",
            island: get("island") == "true",
            island_linger: get("island_linger").parse().unwrap_or(15),
            island_at: IslandPlace::parse(&get("island_at")),
            island_locked: get("island_locked") == "true",
            phone_notifications: get("phone_notifications") != "false",
            welcome_done: get("welcome_done") == "true",
            device_name: get("device_name"),
            bt_phone: get("bt_phone"),
            bt_talk_on_answer: get("bt_talk_on_answer") != "false",
        }
    }

    pub fn render(&self) -> String {
        // Values never contain new lines (paths with one are not worth supporting).
        let clean = |v: &str| v.replace(['\n', '\r'], "");
        format!(
            "# Broche para Linux: preferencias\nlang={}\ntheme={}\nshared_dir={}\nportal_token={}\nexperimental60={}\nvideo_at={}\n\
             scheme={}\ntext_scale={}\nclose_quits={}\nstorage_list={}\nflatpak_autostart={}\nclipboard_auto={}\nhide_tips={}\nisland={}\nisland_linger={}\nisland_at={}\nisland_locked={}\nphone_notifications={}\nwelcome_done={}\ndevice_name={}\nbt_phone={}\nbt_talk_on_answer={}\n",
            clean(&self.lang),
            clean(&self.theme),
            clean(&self.shared_dir),
            clean(&self.portal_token),
            self.experimental60,
            self.video_at,
            clean(&self.scheme),
            self.text_scale,
            self.close_quits,
            self.storage_list,
            self.flatpak_autostart,
            self.clipboard_auto,
            self.hide_tips,
            self.island,
            self.island_linger,
            self.island_at.render(),
            self.island_locked,
            self.phone_notifications,
            self.welcome_done,
            clean(&self.device_name),
            clean(&self.bt_phone),
            self.bt_talk_on_answer,
        )
    }

    pub fn load() -> Self {
        path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, self.render());
    }

    /// Changes one value and saves, reading the file first so values written by
    /// another part of the app (the portal token) are kept.
    pub fn update(change: impl FnOnce(&mut Settings)) {
        let mut s = Self::load();
        change(&mut s);
        s.save();
    }
}

/// Where the island goes: a place on the main screen, worked out for its size each time, or
/// where it was dragged (the middle of its top edge, in screen points).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IslandPlace {
    #[default]
    TopCentre,
    TopLeft,
    TopRight,
    Bottom,
    At(i32, i32),
}

impl IslandPlace {
    fn parse(text: &str) -> Self {
        match text.trim() {
            "top-left" => Self::TopLeft,
            "top-right" => Self::TopRight,
            "bottom" => Self::Bottom,
            other => other
                .split_once(',')
                .and_then(|(x, y)| Some(Self::At(x.trim().parse().ok()?, y.trim().parse().ok()?)))
                .unwrap_or_default(),
        }
    }

    fn render(self) -> String {
        match self {
            Self::TopCentre => String::new(),
            Self::TopLeft => "top-left".into(),
            Self::TopRight => "top-right".into(),
            Self::Bottom => "bottom".into(),
            Self::At(x, y) => format!("{x},{y}"),
        }
    }
}

/// How the app was installed: it decides how autostart works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packaging {
    /// Inside a Flatpak sandbox: autostart goes through the Background portal.
    Flatpak,
    /// An AppImage: the autostart entry must point at the .AppImage file, not at the
    /// temporary mount it runs from.
    AppImage,
    Plain,
}

pub fn packaging() -> Packaging {
    if std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists() {
        Packaging::Flatpak
    } else if std::env::var_os("APPIMAGE").is_some() {
        Packaging::AppImage
    } else {
        Packaging::Plain
    }
}

/// The XDG autostart entry that opens the app (minimized) when the user logs in. Not named
/// after the app ID: that name belongs to the Flatpak's entry, which the Background portal
/// writes, and sharing it let one build switch the other's autostart off.
fn autostart_file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("autostart").join("broche.desktop"))
}

pub fn autostart_enabled() -> bool {
    if packaging() == Packaging::Flatpak {
        // The portal doesn't say; remember what we asked for.
        return Settings::load().flatpak_autostart;
    }
    autostart_file().is_some_and(|p| p.exists())
}

/// Asks the Background portal to start Broche with the session or not (Flatpak). Async on the
/// app's runtime: waiting for it on the window's thread froze the window.
pub async fn request_background(on: bool) -> Result<(), String> {
    use ashpd::desktop::background::Background;
    let reason = crate::i18n::t("Keep your phone connected after you log in");
    Background::request()
        .reason(reason.as_str())
        .auto_start(on)
        .command(["broche", "--minimized"])
        .dbus_activatable(false)
        .send()
        .await
        .and_then(|r| r.response())
        .map_err(|e| e.to_string())?;
    Settings::update(|s| s.flatpak_autostart = on);
    Ok(())
}

/// Turns autostart on or off. In a Flatpak this asks the Background portal (the desktop
/// may show a dialog); elsewhere it writes `~/.config/autostart/broche.desktop`.
pub fn set_autostart(on: bool) -> std::io::Result<()> {
    if packaging() == Packaging::Flatpak {
        return Err(std::io::Error::other("in a Flatpak use request_background"));
    }
    let Some(file) = autostart_file() else { return Err(std::io::Error::other("no config folder")) };
    if !on {
        return match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let exe = match std::env::var_os("APPIMAGE") {
        Some(appimage) => PathBuf::from(appimage),
        None => std::env::current_exe()?,
    };
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        file,
        format!(
            "[Desktop Entry]\nType=Application\nName=Broche\nComment=Keeps your phone connected\n\
             Exec=\"{}\" --minimized\nIcon=io.github.robertcjla.Broche\nTerminal=false\n\
             X-GNOME-Autostart-enabled=true\nX-GNOME-Autostart-Delay=3\n",
            exe.display()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_is_safe_by_default() {
        assert_eq!(Settings::default().video_fps(), 30);
        assert_eq!(Settings::parse("experimental60=true").video_fps(), 60);
        assert_eq!(Settings::parse("theme=dark").video_fps(), 30);
    }

    #[test]
    fn bluetooth_audio_defaults_to_automatic_and_can_be_disabled() {
        assert!(Settings::default().bt_talk_on_answer);
        let s = Settings::parse("bt_talk_on_answer=false");
        assert!(!s.bt_talk_on_answer);
        assert_eq!(Settings::parse(&s.render()), s);
    }

    #[test]
    fn bluetooth_phone_round_trip() {
        let s = Settings::parse("bt_phone=00:11:22:33:44:55");
        assert_eq!(s.bt_phone, "00:11:22:33:44:55");
        assert_eq!(Settings::parse(&s.render()), s);
    }

    #[test]
    fn round_trip() {
        let s = Settings {
            lang: "es".into(),
            theme: "dark".into(),
            shared_dir: "/home/x/Comp=artida".into(),
            portal_token: "abc".into(),
            experimental60: false,
            video_at: 1_793_800_000_000,
            scheme: "tokyo-moon".into(),
            text_scale: 115,
            close_quits: true,
            storage_list: true,
            flatpak_autostart: false,
            clipboard_auto: true,
            hide_tips: true,
            island: true,
            island_linger: 30,
            island_at: IslandPlace::At(640, -12),
            island_locked: true,
            phone_notifications: false,
            welcome_done: true,
            device_name: "Portátil de Ana".into(),
            bt_phone: "00:11:22:33:44:55".into(),
            bt_talk_on_answer: true,
        };
        assert_eq!(Settings::parse(&s.render()), s);
        for place in
            [IslandPlace::TopCentre, IslandPlace::TopLeft, IslandPlace::TopRight, IslandPlace::Bottom]
        {
            assert_eq!(IslandPlace::parse(&place.render()), place);
        }
        assert_eq!(IslandPlace::parse("nonsense"), IslandPlace::TopCentre);
        assert_eq!(Settings::parse("garbage\n"), Settings::default());
        assert_eq!(Settings::default().zoom(), 1.0);
        assert_eq!(Settings::parse("text_scale=500").zoom(), 1.6);
    }
}
