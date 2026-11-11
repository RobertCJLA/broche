// SPDX-License-Identifier: GPL-3.0-or-later
//! The phone's notifications as desktop notifications too (Preferences › Notifications): the
//! ones that popped up on the phone, with the app's icon. An update replaces its notification,
//! one gone from the phone goes from the desktop, and clicking one opens Broche's page.
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use broche_core::{DeviceId, Notification};
use futures_util::StreamExt;
use zbus::zvariant::Value;

use crate::Msg;

static ON: AtomicBool = AtomicBool::new(true);
/// The desktop's ID for each phone notification shown, by phone and key.
static SHOWN: Mutex<Option<HashMap<(DeviceId, String), u32>>> = Mutex::new(None);
/// Each app's icon (PNG): the phone sends it with the app's first notification only.
static ICONS: Mutex<Option<HashMap<String, Vec<u8>>>> = Mutex::new(None);

pub fn set_enabled(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// What the desktop shows: "App · title" and the text, markup-safe (GNOME reads the body as
/// markup).
fn texts(n: &Notification) -> (String, String) {
    let summary = match n.title.as_deref().filter(|t| !t.is_empty()) {
        Some(title) => format!("{} · {title}", n.app_name),
        None => n.app_name.clone(),
    };
    let body = n.text.clone().unwrap_or_default();
    let body = body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    (summary, body)
}

/// Only what popped up on the phone: not silent ones, nor ongoing ones (music, navigation,
/// progress), which would fill the desktop.
fn worth_showing(n: &Notification) -> bool {
    !n.silent && n.live.is_none()
}

/// One at a time: the phone often posts a notification twice in a row, and the second must
/// replace the first, which needs the desktop's ID for it.
static ORDER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn posted(device: DeviceId, n: &Notification) {
    let _turn = ORDER.lock().await;
    if let Some(icon) = &n.icon {
        ICONS.lock().unwrap().get_or_insert_default().insert(n.app.clone(), icon.clone());
    }
    if !ON.load(Ordering::Relaxed) || !worth_showing(n) {
        return;
    }
    let Some(conn) = crate::platform::notify_conn().await else { return };
    let (summary, body) = texts(n);
    let id_key = (device, n.key.clone());
    let replaces = SHOWN.lock().unwrap().get_or_insert_default().get(&id_key).copied().unwrap_or(0);
    let mut hints: HashMap<&str, Value<'_>> =
        [("urgency", Value::from(1u8)), ("desktop-entry", Value::from("io.github.robertcjla.Broche"))].into();
    let icon = ICONS.lock().unwrap().get_or_insert_default().get(&n.app).cloned();
    if let Some(img) = icon.and_then(|png| image::load_from_memory(&png).ok()) {
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width() as i32, rgba.height() as i32);
        hints.insert("image-data", Value::from((w, h, w * 4, true, 8i32, 4i32, rgba.into_raw())));
    }
    let reply = conn
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                n.app_name.as_str(),
                replaces,
                "io.github.robertcjla.Broche",
                summary.as_str(),
                body.as_str(),
                vec!["default", ""],
                hints,
                -1i32,
            ),
        )
        .await;
    if let Ok(id) = reply.and_then(|r| r.body().deserialize::<u32>()) {
        SHOWN.lock().unwrap().get_or_insert_default().insert(id_key, id);
    }
}

pub async fn removed(device: DeviceId, key: &str) {
    let _turn = ORDER.lock().await;
    let Some(id) = SHOWN.lock().unwrap().get_or_insert_default().remove(&(device, key.to_owned())) else {
        return;
    };
    let Some(conn) = crate::platform::notify_conn().await else { return };
    let _ = conn
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "CloseNotification",
            &(id,),
        )
        .await;
}

/// Clicking one of them opens Broche's Notifications page. Runs for the app's lifetime.
pub async fn listen(ui: tokio::sync::mpsc::UnboundedSender<Msg>, ctx: eframe::egui::Context) {
    let Some(conn) = crate::platform::notify_conn().await else { return };
    let Ok(proxy) = zbus::Proxy::new(
        conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    else {
        return;
    };
    let Ok(mut clicks) = proxy.receive_signal("ActionInvoked").await else { return };
    while let Some(signal) = clicks.next().await {
        let Ok((id, _action)) = signal.body().deserialize::<(u32, String)>() else { continue };
        let ours = SHOWN.lock().unwrap().get_or_insert_default().values().any(|v| *v == id);
        if ours {
            let _ = ui.send(Msg::OpenNotifications);
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(title: Option<&str>, text: &str) -> Notification {
        serde_json::from_value(serde_json::json!({
            "key": "k", "app": "com.example", "app_name": "Chat", "title": title, "text": text, "when": 0,
        }))
        .unwrap()
    }

    #[test]
    fn app_and_title_then_escaped_text() {
        let (summary, body) = texts(&note(Some("Ana"), "a < b & c"));
        assert_eq!(summary, "Chat · Ana");
        assert_eq!(body, "a &lt; b &amp; c");
        assert_eq!(texts(&note(None, "")).0, "Chat");
    }

    #[test]
    fn silent_ones_stay_on_the_phone() {
        let mut n = note(Some("Ana"), "hola");
        assert!(worth_showing(&n));
        n.silent = true;
        assert!(!worth_showing(&n));
    }
}
