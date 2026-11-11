// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux players expose their state and commands through MPRIS on the session bus.
use broche_core::{MediaAction, MediaPlayer, Node};
use std::collections::HashMap;
use zbus::{Connection, Proxy, zvariant::OwnedValue};

pub async fn players(conn: &Connection) -> Result<Vec<MediaPlayer>, String> {
    let bus = zbus::fdo::DBusProxy::new(conn).await.map_err(|e| e.to_string())?;
    let names = bus.list_names().await.map_err(|e| e.to_string())?;
    let mut out = vec![];
    for name in names {
        if !name.as_str().starts_with("org.mpris.MediaPlayer2.") {
            continue;
        }
        let Ok(p) =
            Proxy::new(conn, name.as_str(), "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player").await
        else {
            continue;
        };
        let Ok(status) = p.get_property::<String>("PlaybackStatus").await else { continue };
        let m = p.get_property::<HashMap<String, OwnedValue>>("Metadata").await.unwrap_or_default();
        let text = |key: &str| m.get(key).and_then(|v| <&str>::try_from(v).ok()).map(str::to_owned);
        let artist = m
            .get("xesam:artist")
            .and_then(|v| v.try_clone().ok())
            .and_then(|v| Vec::<String>::try_from(v).ok())
            .map(|v| v.join(", "));
        let art = match text("mpris:artUrl") {
            Some(url) => art(&url).await,
            None => None,
        };
        let micros = m.get("mpris:length").and_then(|v| i64::try_from(v).ok());
        let root = Proxy::new(conn, name.as_str(), "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2")
            .await
            .map_err(|e| e.to_string())?;
        let app = root.get_property::<String>("Identity").await.unwrap_or_else(|_| name.to_string());
        let mut actions = vec![];
        for (prop, action) in
            [("CanPlay", "play"), ("CanPause", "pause"), ("CanGoNext", "next"), ("CanGoPrevious", "previous")]
        {
            if p.get_property::<bool>(prop).await.unwrap_or(false) {
                actions.push(action.to_string());
            }
        }
        out.push(MediaPlayer {
            id: name.to_string(),
            app,
            title: text("xesam:title"),
            artist,
            album: text("xesam:album"),
            playing: status == "Playing",
            duration: micros.map(|v| v.max(0) as u64 / 1000),
            position: None,
            actions,
            art_key: art.as_ref().map(|a| a.0.clone()),
            art: art.map(|a| a.1),
        });
    }
    out.sort_by(|a, b| b.playing.cmp(&a.playing).then(a.id.cmp(&b.id)));
    Ok(out)
}

/// A picture's key and its JPEG.
type Art = (String, Vec<u8>);

/// Biggest picture read or downloaded before shrinking it.
const ART_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// A player's `mpris:artUrl` as (key, small JPEG). Remembered by address, failures too,
/// so the poll every 2 s reads or downloads each picture once.
async fn art(url: &str) -> Option<Art> {
    static CACHE: std::sync::Mutex<Vec<(String, Option<Art>)>> = std::sync::Mutex::new(Vec::new());
    if let Some((_, found)) = CACHE.lock().unwrap().iter().find(|(u, _)| u == url) {
        return found.clone();
    }
    let found = load_art(url).await;
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= 8 {
        cache.remove(0);
    }
    cache.push((url.to_owned(), found.clone()));
    found
}

async fn load_art(url: &str) -> Option<Art> {
    let parsed = url::Url::parse(url).ok()?;
    let bytes = match parsed.scheme() {
        "file" => {
            let path = parsed.to_file_path().ok()?;
            if crate::settings::packaging() == crate::settings::Packaging::Flatpak {
                // The sandbox has its own /tmp, and browsers leave their art in the host's.
                let out = crate::platform::host("head")
                    .arg("-c")
                    .arg((ART_SOURCE_BYTES + 1).to_string())
                    .arg("--")
                    .arg(&path)
                    .output()
                    .ok()?;
                (out.status.success() && out.stdout.len() <= ART_SOURCE_BYTES).then_some(out.stdout)?
            } else {
                if tokio::fs::metadata(&path).await.ok()?.len() > ART_SOURCE_BYTES as u64 {
                    return None;
                }
                tokio::fs::read(path).await.ok()?
            }
        }
        "http" | "https" => download(parsed).await?,
        _ => return None,
    };
    let jpeg = shrink(&bytes)?;
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    Some((format!("{:016x}", h.finish()), jpeg))
}

async fn download(url: url::Url) -> Option<Vec<u8>> {
    static CLIENT: std::sync::OnceLock<Option<reqwest::Client>> = std::sync::OnceLock::new();
    let client = CLIENT
        .get_or_init(|| {
            // reqwest is built without a TLS provider of its own; ring is already compiled in.
            let _ = rustls::crypto::ring::default_provider().install_default();
            reqwest::Client::builder()
                .user_agent(concat!("Broche/", env!("CARGO_PKG_VERSION")))
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .ok()
        })
        .as_ref()?;
    let mut resp = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let mut out = vec![];
    while let Some(chunk) = resp.chunk().await.ok()? {
        out.extend_from_slice(&chunk);
        if out.len() > ART_SOURCE_BYTES {
            return None;
        }
    }
    Some(out)
}

/// The picture as a JPEG of at most 320 px, well under what the protocol carries.
fn shrink(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?.thumbnail(320, 320).to_rgb8();
    let mut out = std::io::Cursor::new(vec![]);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&img).ok()?;
    let out = out.into_inner();
    // The phone drops pictures over 256 KiB (`media_limits::ART_BYTES`).
    (out.len() <= 256 * 1024).then_some(out)
}

pub async fn command(player: &str, action: MediaAction, position: Option<u64>) -> Result<(), String> {
    if !player.starts_with("org.mpris.MediaPlayer2.") {
        return Err("Unknown media player".into());
    }
    let conn = Connection::session().await.map_err(|e| e.to_string())?;
    let p = Proxy::new(&conn, player, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player")
        .await
        .map_err(|e| e.to_string())?;
    match action {
        MediaAction::Seek => {
            let m: HashMap<String, OwnedValue> =
                p.get_property("Metadata").await.map_err(|e| e.to_string())?;
            let track = m
                .get("mpris:trackid")
                .and_then(|v| v.try_clone().ok())
                .and_then(|v| zbus::zvariant::OwnedObjectPath::try_from(v).ok())
                .ok_or("Track has no ID")?;
            p.call::<_, _, ()>(
                "SetPosition",
                &(track, (position.unwrap_or(0).saturating_mul(1000).min(i64::MAX as u64)) as i64),
            )
            .await
        }
        other => {
            p.call::<_, _, ()>(
                match other {
                    MediaAction::Play => "Play",
                    MediaAction::Pause => "Pause",
                    MediaAction::Next => "Next",
                    MediaAction::Previous => "Previous",
                    _ => unreachable!(),
                },
                &(),
            )
            .await
        }
    }
    .map_err(|e| e.to_string())
}

/// What the last poll found, for the island.
static LAST: std::sync::Mutex<Vec<MediaPlayer>> = std::sync::Mutex::new(Vec::new());

/// This PC's players as of the last poll (every 2 s).
pub fn last() -> Vec<MediaPlayer> {
    LAST.lock().unwrap().clone()
}

pub async fn watch(node: Node) {
    let Ok(conn) = Connection::session().await else { return };
    let mut last = vec![];
    loop {
        if let Ok(next) = players(&conn).await
            && next != last
        {
            *LAST.lock().unwrap() = next.clone();
            node.media_changed(next.clone()).await;
            last = next;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_art_is_shrunk_to_a_small_jpeg() {
        let img =
            image::RgbImage::from_fn(1200, 900, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]));
        let mut png = std::io::Cursor::new(vec![]);
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let jpeg = shrink(png.get_ref()).unwrap();
        let back = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((back.width(), back.height()), (320, 240));
        assert!(shrink(b"not a picture").is_none());
    }

    #[tokio::test]
    async fn art_from_a_file_url_and_unknown_schemes() {
        let dir = std::env::temp_dir().join(format!("broche-art-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cover.png");
        image::RgbImage::new(40, 40).save(&path).unwrap();
        let url = url::Url::from_file_path(&path).unwrap().to_string();
        let (key, jpeg) = art(&url).await.unwrap();
        assert!(!key.is_empty() && key.len() <= 64 && !jpeg.is_empty());
        assert!(art("ftp://example.com/cover.png").await.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Player(Arc<AtomicBool>);
    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl Player {
        fn play(&self) {
            self.0.store(true, Ordering::SeqCst);
        }
        fn pause(&self) {
            self.0.store(false, Ordering::SeqCst);
        }
        #[zbus(property)]
        fn playback_status(&self) -> &str {
            if self.0.load(Ordering::SeqCst) { "Playing" } else { "Paused" }
        }
        #[zbus(property)]
        fn can_play(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_pause(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_go_next(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn can_go_previous(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn metadata(&self) -> HashMap<String, OwnedValue> {
            HashMap::from([(
                "xesam:title".into(),
                OwnedValue::from(zbus::zvariant::Str::from("Prueba de audio")),
            )])
        }
    }
    struct Root;
    #[zbus::interface(name = "org.mpris.MediaPlayer2")]
    impl Root {
        #[zbus(property)]
        fn identity(&self) -> &str {
            "Test player"
        }
    }

    #[tokio::test]
    #[ignore = "Run under dbus-run-session to avoid using the user's media players"]
    async fn discovers_and_controls_mpris_player() {
        let playing = Arc::new(AtomicBool::new(false));
        let id = "org.mpris.MediaPlayer2.broche_test";
        let _server = zbus::connection::Builder::session()
            .unwrap()
            .name(id)
            .unwrap()
            .serve_at("/org/mpris/MediaPlayer2", Player(playing.clone()))
            .unwrap()
            .serve_at("/org/mpris/MediaPlayer2", Root)
            .unwrap()
            .build()
            .await
            .unwrap();
        let conn = Connection::session().await.unwrap();
        let found = players(&conn).await.unwrap();
        let p = found.iter().find(|p| p.id == id).unwrap();
        assert_eq!(p.title.as_deref(), Some("Prueba de audio"));
        assert!(!p.playing);
        command(id, MediaAction::Play, None).await.unwrap();
        assert!(playing.load(Ordering::SeqCst));
        command(id, MediaAction::Pause, None).await.unwrap();
        assert!(!playing.load(Ordering::SeqCst));
    }
}
