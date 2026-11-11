// SPDX-License-Identifier: GPL-3.0-or-later
//! The phone's sound on this PC over Bluetooth (A2DP): the phone plays to the PC as to a speaker.
//! Broche only connects or releases that one profile of a phone already paired in the system's
//! Bluetooth settings; calls (hands-free) and the pairing itself are left alone. Android pauses
//! its player when the output changes, so the caller resumes it.
use crate::i18n::t;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};
use zbus::{Connection, Proxy, proxy::MethodFlags, zvariant::OwnedValue};

/// The phone's A2DP source profile: what Broche asks the phone for, or releases.
const A2DP_SOURCE: &str = "0000110a-0000-1000-8000-00805f9b34fb";
/// This PC's A2DP sink: a transport of the phone with it means its sound plays here.
const A2DP_SINK: &str = "0000110b-0000-1000-8000-00805f9b34fb";

/// A phone paired over Bluetooth that can send its sound to this PC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone {
    /// BlueZ object path of the device.
    pub path: String,
    pub address: String,
    pub connected: bool,
    /// Its sound plays on this PC now.
    pub on: bool,
}

static STATE: RwLock<Option<Phone>> = RwLock::new(None);
/// A switch is under way: another one (a repeated press, Home and the tray at once) is ignored.
static BUSY: AtomicBool = AtomicBool::new(false);
/// Until then [`watch`] keeps what the last switch set: BlueZ takes a moment to add or drop the
/// transport, and showing that passing state would flip the switch back and forth.
static SETTLING_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

/// Claims the switch before starting one; false while another is under way.
pub fn claim() -> bool {
    !BUSY.swap(true, Ordering::AcqRel)
}

/// A switch is under way (the switch in Home waits for it).
pub fn busy() -> bool {
    BUSY.load(Ordering::Acquire)
}

/// The phone, as last seen; None when no paired device can send sound (or BlueZ is missing).
pub fn current() -> Option<Phone> {
    STATE.read().unwrap().clone()
}

/// Keeps [`current`] up to date while Broche runs.
pub async fn watch() {
    let mut conn = None;
    loop {
        if conn.is_none() {
            conn = Connection::system().await.ok();
        }
        let phone = match &conn {
            Some(c) => find(c).await.unwrap_or_else(|_| {
                conn = None; // BlueZ isn't running: look again later.
                None
            }),
            None => None,
        };
        let settling = SETTLING_UNTIL.lock().unwrap().is_some_and(|t| Instant::now() < t);
        if !busy() && !settling {
            *STATE.write().unwrap() = phone;
        }
        // Without BlueZ (no Bluetooth on this PC) there is nothing to follow closely.
        tokio::time::sleep(Duration::from_secs(if conn.is_some() { 3 } else { 30 })).await;
    }
}

/// Sends the phone's sound to this PC (`on`) or back to the phone; false when it already was.
pub async fn set(on: bool) -> Result<bool, String> {
    let phone = current().ok_or_else(|| t("No phone is paired with this PC over Bluetooth."))?;
    let conn = Connection::system().await.map_err(|e| e.to_string())?;
    let device = Proxy::new(&conn, "org.bluez", phone.path.as_str(), "org.bluez.Device1")
        .await
        .map_err(|e| e.to_string())?;
    let method = if on { "ConnectProfile" } else { "DisconnectProfile" };
    let result: zbus::Result<()> = device.call(method, &(A2DP_SOURCE,)).await;
    let changed = match result {
        Ok(()) => true,
        // Already as asked: nothing to do.
        Err(zbus::Error::MethodError(name, _, _))
            if matches!(
                name.as_str(),
                "org.bluez.Error.AlreadyConnected" | "org.bluez.Error.NotConnected"
            ) =>
        {
            false
        }
        Err(e) => {
            eprintln!("Bluetooth sound {method}: {e}");
            return Err(match &e {
                zbus::Error::MethodError(name, _, _) if name.as_str() == "org.bluez.Error.InProgress" => {
                    t("The phone is still connecting over Bluetooth: try again in a moment.")
                }
                // BlueZ has no sound service to hand the phone's sound to (PipeWire not running).
                zbus::Error::MethodError(_, Some(detail), _) if detail.contains("profile-unavailable") => {
                    t("This PC can't receive Bluetooth sound right now.")
                }
                // Usually the phone's Bluetooth is off or it is out of range.
                _ => t("The phone didn't answer over Bluetooth: check that its Bluetooth is on."),
            });
        }
    };
    // Show the result now; BlueZ catches up in a moment.
    *SETTLING_UNTIL.lock().unwrap() = Some(Instant::now() + Duration::from_secs(4));
    if let Some(phone) = STATE.write().unwrap().as_mut() {
        phone.on = on;
    }
    Ok(changed)
}

/// [`set`], then plays `resume` (the phone's player that was playing) again: Android pauses it
/// when the output changes. What to tell the user, or why it failed. Call after [`claim`].
pub async fn switch(
    on: bool,
    resume: Option<(broche_core::Node, broche_core::DeviceId, String)>,
) -> Result<String, String> {
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::Release);
        }
    }
    let _release = Release;
    if set(on).await?
        && let Some((node, peer, player)) = resume
    {
        // Give the new output a moment before playing again.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let _ = node.media_command(peer, player, broche_core::MediaAction::Play, None).await;
    }
    Ok(t(if on { "The phone's sound plays on this PC." } else { "The phone's sound is back on the phone." }))
}

async fn find(conn: &Connection) -> zbus::Result<Option<Phone>> {
    let objects: zbus::fdo::ManagedObjects =
        Proxy::new(conn, "org.bluez", "/", "org.freedesktop.DBus.ObjectManager")
            .await?
            // Don't start BlueZ just to look: without it there is no phone to find.
            .call_with_flags("GetManagedObjects", MethodFlags::NoAutoStart.into(), &())
            .await?
            .unwrap_or_default();
    let text = crate::bt_calls::text;
    let flag = |p: &std::collections::HashMap<String, OwnedValue>, k: &str| {
        p.get(k).and_then(|v| bool::try_from(v).ok()).unwrap_or(false)
    };
    let mut devices = Vec::new();
    let mut playing_here = Vec::new();
    for (path, interfaces) in &objects {
        if let Some(d) = interfaces.get("org.bluez.Device1") {
            let uuids: Vec<String> =
                d.get("UUIDs").and_then(|v| Vec::<String>::try_from(v.clone()).ok()).unwrap_or_default();
            if flag(d, "Paired") && uuids.iter().any(|u| u.eq_ignore_ascii_case(A2DP_SOURCE)) {
                devices.push(Phone {
                    path: path.to_string(),
                    address: text(d, "Address"),
                    connected: flag(d, "Connected"),
                    on: false,
                });
            }
        }
        if let Some(t) = interfaces.get("org.bluez.MediaTransport1")
            && text(t, "UUID").eq_ignore_ascii_case(A2DP_SINK)
            && let Some(device) =
                t.get("Device").and_then(|v| zbus::zvariant::ObjectPath::try_from(v.clone()).ok())
        {
            playing_here.push(device.to_string());
        }
    }
    Ok(pick(devices, &playing_here, &crate::bt_calls::preferred_phone()))
}

/// The phone chosen for calls, else one connected now, else the first; `on` when one of
/// `playing_here` (devices with a sound transport to this PC) is it.
fn pick(mut devices: Vec<Phone>, playing_here: &[String], preferred: &str) -> Option<Phone> {
    devices.sort_by(|a, b| a.path.cmp(&b.path));
    let index = devices
        .iter()
        .position(|d| !preferred.is_empty() && d.address.eq_ignore_ascii_case(preferred))
        .or_else(|| devices.iter().position(|d| d.connected))
        .or((!devices.is_empty()).then_some(0))?;
    let mut phone = devices.swap_remove(index);
    phone.on = playing_here.contains(&phone.path);
    Some(phone)
}

#[cfg(test)]
mod tests {
    use super::{Phone, pick};

    fn phone(path: &str, address: &str, connected: bool) -> Phone {
        Phone { path: path.into(), address: address.into(), connected, on: false }
    }

    #[test]
    fn picks_the_calls_phone_then_a_connected_one() {
        let devices = vec![phone("/a", "AA", false), phone("/b", "BB", true), phone("/c", "CC", false)];
        assert_eq!(pick(devices.clone(), &[], "cc").unwrap().path, "/c");
        assert_eq!(pick(devices.clone(), &[], "").unwrap().path, "/b");
        assert_eq!(pick(devices.clone(), &[], "ZZ").unwrap().path, "/b");
        assert!(pick(vec![], &[], "").is_none());
        let only_offline = vec![phone("/x", "XX", false)];
        assert_eq!(pick(only_offline, &[], "").unwrap().path, "/x");
    }

    /// Switches a real phone's sound here and back (`cargo test -- --ignored real_phone`): it
    /// must be paired with this PC over Bluetooth and reachable. Leaves it as it was.
    #[test]
    #[ignore = "talks to a real Bluetooth phone"]
    fn real_phone_sound_switches_and_comes_back() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let conn = zbus::Connection::system().await.unwrap();
            let before = super::find(&conn).await.unwrap().expect("a phone paired over Bluetooth");
            *super::STATE.write().unwrap() = Some(before.clone());
            for on in [!before.on, before.on] {
                super::set(on).await.unwrap();
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let now = super::find(&conn).await.unwrap().unwrap();
                assert_eq!(now.on, on, "after asking for on = {on}");
                assert!(now.connected, "the phone stays connected (calls are left alone)");
            }
        });
    }

    #[test]
    fn a_second_switch_waits_for_the_first() {
        assert!(super::claim());
        assert!(!super::claim(), "a repeated press while switching does nothing");
        super::BUSY.store(false, std::sync::atomic::Ordering::Release);
        assert!(super::claim());
        super::BUSY.store(false, std::sync::atomic::Ordering::Release);
    }

    #[test]
    fn on_when_its_sound_plays_here() {
        let devices = vec![phone("/a", "AA", true)];
        assert!(pick(devices.clone(), &["/a".into()], "").unwrap().on);
        assert!(!pick(devices, &["/other".into()], "").unwrap().on);
    }
}
