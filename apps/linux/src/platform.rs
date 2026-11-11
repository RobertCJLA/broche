// SPDX-License-Identifier: GPL-3.0-or-later
//! What this Linux PC does when a paired phone asks: remote mouse and keyboard and
//! screen sharing (through the desktop's RemoteDesktop and ScreenCast portals),
//! volume, phone-as-speaker, ringing, lock, sleep, clipboard, links and the
//! shared folder.

use std::{
    io::Read,
    os::{fd::OwnedFd, unix::process::CommandExt},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use ashpd::desktop::{
    Session,
    remote_desktop::{DeviceType, KeyState, RemoteDesktop},
    screencast::{CursorMode, Screencast, SourceType},
};
use broche_core::{
    ButtonAction, DeckState, DeviceId, FolderStorage, KeyMod, MIRROR_H264, MIRROR_SCREEN, MirrorConfig,
    MirrorInput, MirrorSend, MirrorSink, MirrorStart, MouseButton, Node, PacketKind, Platform, PowerAction,
    RemoteInput, SlideAction, StorageEntry, StorageError, StorageReadFile, StorageWriteDone, TouchAction,
    mirror_keys,
};
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot},
};

use crate::{Msg, i18n::t, mirror::MirrorShared};

/// What the portal gave for capturing the screen.
struct CaptureInfo {
    fd: OwnedFd,
    node: u32,
    size: (i32, i32),
}

enum Cmd {
    Input(RemoteInput),
    Touch { action: TouchAction, x: f32, y: f32 },
    Scroll { x: f32, y: f32, dx: f32, dy: f32 },
    Capture(oneshot::Sender<Result<CaptureInfo, String>>),
}

struct Capture {
    child: Child,
    stop: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct LinuxPlatform {
    pub node: OnceLock<Node>,
    rt: Handle,
    ui: mpsc::UnboundedSender<Msg>,
    cmd: mpsc::UnboundedSender<Cmd>,
    speaker: Arc<Mutex<Option<Child>>>,
    ringing: Mutex<Option<(Child, (u8, bool))>>,
    capture: Arc<Mutex<Option<Capture>>>,
    shared_dir: RwLock<Option<FolderStorage>>,
    pub mirror: Arc<MirrorShared>,
    sink: Arc<crate::mirror::Sink>,
    /// The phone apps shown in windows of their own, by the PC's session number.
    pub app_sinks: Mutex<std::collections::HashMap<u32, Arc<crate::mirror::Sink>>>,
    pub cam: Arc<MirrorShared>,
    cam_sink: Arc<crate::mirror::CamSink>,
    /// The user's own Deck buttons (`crate::deck`).
    pub deck: RwLock<Vec<broche_core::DeckTileConfig>>,
    /// The phone's laser pointer in presentation mode.
    laser: crate::laser::Laser,
}

impl std::fmt::Debug for Cmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cmd")
    }
}

impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Capture")
    }
}

impl LinuxPlatform {
    pub fn new(
        rt: Handle,
        ui: mpsc::UnboundedSender<Msg>,
        mirror: Arc<MirrorShared>,
        cam: Arc<MirrorShared>,
        shared_dir: PathBuf,
    ) -> Arc<Self> {
        let (cmd, rx) = mpsc::unbounded_channel();
        let capture = Arc::new(Mutex::new(None));
        rt.spawn(portal_worker(rx, ui.clone(), Arc::clone(&capture)));
        let sink = crate::mirror::Sink::new("screen", mirror.clone());
        let cam_sink = Arc::new(crate::mirror::CamSink(crate::mirror::Sink::new("camera", cam.clone())));
        let p = Arc::new(Self {
            node: OnceLock::new(),
            rt,
            ui,
            cmd,
            speaker: Arc::new(Mutex::new(None)),
            ringing: Mutex::new(None),
            capture,
            shared_dir: RwLock::new(None),
            mirror,
            sink,
            app_sinks: Mutex::new(Default::default()),
            cam,
            cam_sink,
            deck: RwLock::new(crate::deck::load()),
            laser: crate::laser::Laser::spawn(),
        });
        p.set_shared_dir(shared_dir);
        p
    }

    /// The folder phones may browse (once allowed).
    pub fn set_shared_dir(&self, dir: PathBuf) {
        let _ = std::fs::create_dir_all(&dir);
        *self.shared_dir.write().unwrap() = Some(FolderStorage::new(dir).with_blocked(sensitive_folders()));
    }

    fn storage<T>(
        &self,
        f: impl FnOnce(&FolderStorage) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        match self.shared_dir.read().unwrap().as_ref() {
            Some(s) => f(s),
            None => Err(StorageError::Unsupported),
        }
    }

    /// Publishes this PC's volume, and whether its sound is playing on a phone, so
    /// phones show the right volume and the right state of "Use phone as speaker".
    ///
    /// Called from here, not from the window: the window may not be drawn (minimized,
    /// another workspace), and a state without the phone speaker turns the phone's
    /// switch off.
    pub fn publish_audio_state(&self) {
        let Some(node) = self.node.get().cloned() else { return };
        let speaking = self.speaker.lock().unwrap().is_some();
        publish_deck(&self.rt, node, speaking);
    }

    /// Follows this PC's volume (keyboard keys, the desktop's slider…) and tells phones
    /// when it changes, so their slider matches.
    pub fn watch_volume(self: &Arc<Self>) {
        let this = Arc::downgrade(self);
        std::thread::spawn(move || {
            let mut last = pc_volume();
            loop {
                std::thread::sleep(Duration::from_millis(1500));
                let Some(p) = this.upgrade() else { return };
                let now = pc_volume();
                if now != last {
                    last = now;
                    p.publish_audio_state();
                    let _ = p.ui.send(Msg::AudioChanged);
                }
            }
        });
    }

    /// Stops everything this PC was doing for a phone (on exit).
    pub fn shutdown(&self) {
        self.stop_capture();
        self.stop_speaker();
        self.stop_ringing();
    }

    pub fn stop_ringing_now(&self) {
        self.stop_ringing();
    }

    fn stop_speaker(&self) {
        if let Some(mut c) = self.speaker.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub fn stop_video(&self) {
        self.stop_capture();
    }

    /// This PC's screen is being sent to a phone now.
    pub fn sharing_screen(&self) -> bool {
        self.capture.lock().unwrap().as_ref().is_some_and(|c| !c.stop.load(Ordering::SeqCst))
    }

    fn stop_capture(&self) {
        if let Some(mut c) = self.capture.lock().unwrap().take() {
            c.stop.store(true, Ordering::SeqCst);
            let _ = c.child.kill();
            let _ = c.child.wait();
        }
    }

    fn start_speaker(&self, peer: DeviceId) -> Result<(), String> {
        self.stop_speaker();
        let node = self.node.get().cloned().ok_or("not ready")?;
        let slot = self.speaker.clone();
        let ui = self.ui.clone();
        // What the speakers play: PipeWire's `pw-record` (on every modern desktop), else `parec`.
        let capture = |program: &str, args: &[&str]| {
            Command::new(program).args(args).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        };
        let pw = ["-P", "stream.capture.sink=true", "--raw", "--format=s16", "--rate=48000", "--channels=2"];
        let parec = ["--device=@DEFAULT_MONITOR@", "--format=s16le", "--rate=48000", "--channels=2"];
        let mut child = capture("pw-record", &[&pw[..], &["--latency=20ms", "-"]].concat())
            .or_else(|_| capture("parec", &[&parec[..], &["--latency-msec=20"]].concat()))
            .map_err(|e| format!("can't capture the PC's sound (is pw-record or parec installed?): {e}"))?;
        let mut out = child.stdout.take().ok_or("no audio pipe")?;
        *self.speaker.lock().unwrap() = Some(child);
        let rt = self.rt.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 960];
            let mut failing_since: Option<Instant> = None;
            while out.read_exact(&mut buf).is_ok() {
                if rt.block_on(node.send_pc_audio_pcm(&peer, 48_000, 2, buf.to_vec())).is_ok() {
                    failing_since = None;
                    continue;
                }
                // The phone went away: give up after a few seconds instead of
                // capturing sound for nobody.
                let since = *failing_since.get_or_insert_with(Instant::now);
                if since.elapsed() > Duration::from_secs(4) {
                    if let Some(mut c) = slot.lock().unwrap().take() {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                    publish_deck(&rt, node.clone(), false);
                    let _ = ui.send(Msg::SpeakerChanged(false));
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        Ok(())
    }

    /// Captures this PC's screen and streams it to the phone that asked.
    fn start_capture(&self, peer: DeviceId, options: &MirrorStart) -> Result<(), String> {
        self.stop_capture();
        let node = self.node.get().cloned().ok_or("not ready")?;
        // Testing aid: BROCHE_TEST_SCREEN=1 sends a test picture instead of the real
        // screen, so the path to the phone can be checked without the desktop's prompt.
        let test = std::env::var_os("BROCHE_TEST_SCREEN").is_some();
        let rx = if test {
            None
        } else {
            let (tx, rx) = oneshot::channel();
            self.cmd.send(Cmd::Capture(tx)).map_err(|_| "screen sharing is not available".to_string())?;
            Some(rx)
        };
        let rt = self.rt.clone();
        let ui = self.ui.clone();
        let options = options.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let slot = self.capture.clone();
        // The portal may ask the user, so wait for it off the caller's thread.
        std::thread::spawn(move || {
            let (source, size, fd) = match rx {
                None => ("videotestsrc is-live=true pattern=ball".to_string(), (1280, 720), None),
                Some(rx) => {
                    let info = match rt.block_on(rx) {
                        Ok(Ok(i)) => i,
                        Ok(Err(e)) => {
                            let _ = ui.send(Msg::Line(format!("{} {e}", t("Screen sharing failed:"))));
                            return;
                        }
                        Err(_) => return,
                    };
                    if let Err(e) = rustix::io::fcntl_setfd(&info.fd, rustix::io::FdFlags::empty()) {
                        let _ = ui.send(Msg::Line(format!("{} {e}", t("Screen sharing failed:"))));
                        return;
                    }
                    let src = format!(
                        "pipewiresrc fd={} path={} do-timestamp=true",
                        std::os::fd::AsRawFd::as_raw_fd(&info.fd),
                        info.node
                    );
                    (src, info.size, Some(info.fd))
                }
            };
            let (w, h) = target_size(size, options.max_size);
            let fps = options.fps.clamp(1, crate::settings::Settings::load().video_fps());
            let kbps = (options.bitrate / 1000).clamp(500, 20_000);
            let Some((_, format, encoder)) = h264_encoder() else {
                let _ =
                    ui.send(Msg::Line(t("GStreamer has no H.264 encoder: install openh264 or x264 for it.")));
                return;
            };
            let encoder = fill_encoder(encoder, kbps, fps);
            let pipeline = format!(
                "{source} ! videorate ! video/x-raw,framerate={fps}/1 \
                 ! videoconvert ! videoscale ! video/x-raw,width={w},height={h},format={format} \
                 ! {encoder} \
                 ! h264parse config-interval=-1 ! video/x-h264,stream-format=byte-stream,alignment=au \
                 ! fdsink fd=1",
            );
            let spawned = Command::new("gst-launch-1.0")
                .arg("-q")
                .args(pipeline.split_whitespace())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let child = match spawned {
                Ok(c) => c,
                Err(e) => {
                    let _ = ui.send(Msg::Line(format!("{} {e}", t("Can't run gst-launch-1.0 (GStreamer):"))));
                    return;
                }
            };
            drop(fd);
            let mut child = child;
            let Some(mut out) = child.stdout.take() else { return };
            *slot.lock().unwrap() = Some(Capture { child, stop: stop2.clone() });
            let sender = match rt.block_on(node.mirror_open(peer)) {
                Ok(s) => s,
                Err(e) => {
                    let _ = ui.send(Msg::Line(format!("{} {e}", t("Can't open the screen stream:"))));
                    return;
                }
            };
            std::thread::sleep(Duration::from_millis(150));
            let cfg = MirrorConfig { codec: MIRROR_H264.into(), width: w, height: h, session: MIRROR_SCREEN };
            if matches!(sender.send(PacketKind::Config, 0, cfg.to_cbor()), MirrorSend::Closed) {
                return;
            }
            let _ = ui.send(Msg::Line(format!("{} ({w}x{h}).", t("Sharing this PC's screen"))));
            let started = Instant::now();
            let mut buf: Vec<u8> = Vec::with_capacity(1 << 20);
            let mut chunk = vec![0u8; 1 << 16];
            let mut waiting_key = false;
            loop {
                if stop2.load(Ordering::SeqCst) {
                    break;
                }
                let n = match out.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                buf.extend_from_slice(&chunk[..n]);
                for au in take_access_units(&mut buf) {
                    let key = au_has_idr(&au);
                    if waiting_key && !key {
                        continue;
                    }
                    waiting_key = false;
                    let kind = if key { PacketKind::Keyframe } else { PacketKind::Frame };
                    let time_us = started.elapsed().as_micros() as u64;
                    match sender.send(kind, time_us, au) {
                        MirrorSend::Closed => {
                            stop2.store(true, Ordering::SeqCst);
                        }
                        MirrorSend::NeedKeyframe => waiting_key = true,
                        MirrorSend::Queued | MirrorSend::Dropped => {}
                    }
                }
            }
            // However it ended, it no longer needs the desktop session (see portal_worker).
            stop2.store(true, Ordering::SeqCst);
            sender.close();
        });
        let _ = stop;
        Ok(())
    }
}

/// H.264 encoders GStreamer may have, in order: distributions ship different ones (Debian and
/// Ubuntu have no openh264, x264 is in "ugly", the VA ones use the GPU). `{b}` bit/s, `{k}` kbit/s,
/// `{g}` frames between keyframes.
/// shortcut: only openh264enc was tried on real hardware; x264enc and the VA encoders follow their
/// documented properties, check them on a distribution that has them.
const H264_ENCODERS: [(&str, &str, &str); 4] = [
    (
        "openh264enc",
        "I420",
        "openh264enc bitrate={b} gop-size={g} complexity=low rate-control=bitrate usage-type=screen",
    ),
    (
        "x264enc",
        "I420",
        "x264enc bitrate={k} key-int-max={g} tune=zerolatency speed-preset=ultrafast byte-stream=true",
    ),
    ("vah264enc", "NV12", "vah264enc bitrate={k} key-int-max={g} rate-control=cbr"),
    ("vaapih264enc", "NV12", "vaapih264enc bitrate={k} keyframe-period={g} rate-control=cbr"),
];

/// Whether GStreamer has any H.264 encoder this app can use (for About).
pub fn has_h264_encoder() -> bool {
    h264_encoder().is_some()
}

/// The first encoder of [`H264_ENCODERS`] this system has (looked up once).
fn h264_encoder() -> Option<&'static (&'static str, &'static str, &'static str)> {
    static FOUND: OnceLock<Option<usize>> = OnceLock::new();
    let found = *FOUND.get_or_init(|| {
        H264_ENCODERS.iter().position(|(name, _, _)| {
            Command::new("gst-inspect-1.0").args(["--exists", name]).status().is_ok_and(|s| s.success())
        })
    });
    found.map(|i| &H264_ENCODERS[i])
}

fn fill_encoder(template: &str, kbps: u32, fps: u32) -> String {
    template
        .replace("{b}", &(kbps * 1000).to_string())
        .replace("{k}", &kbps.to_string())
        .replace("{g}", &fps.to_string())
}

fn target_size(src: (i32, i32), max_size: u32) -> (u32, u32) {
    let max_edge = f64::from(max_size.clamp(320, 1920));
    let (sw, sh) = (f64::from(src.0.max(64)), f64::from(src.1.max(64)));
    let scale = if sw.max(sh) > max_edge { max_edge / sw.max(sh) } else { 1.0 };
    let w = (((sw * scale).round() as u32) / 16).max(4) * 16;
    let h = (((sh * scale).round() as u32) / 16).max(4) * 16;
    (w, h)
}

/// Offsets of the access-unit delimiters (`00 00 01 09` or `00 00 00 01 09`).
fn aud_positions(buf: &[u8]) -> Vec<usize> {
    let mut out = vec![];
    let mut i = 0;
    while i + 4 < buf.len() {
        if buf[i] == 0 && buf[i + 1] == 0 && buf[i + 2] == 1 && buf[i + 3] & 0x1f == 9 {
            out.push(if i > 0 && buf[i - 1] == 0 { i - 1 } else { i });
            i += 4;
        } else {
            i += 1;
        }
    }
    out
}

/// Takes the complete access units out of `buf` (each runs from one delimiter to the
/// next) and leaves the unfinished one, which the next read completes.
fn take_access_units(buf: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let starts = aud_positions(buf);
    if starts.len() < 2 {
        return Vec::new();
    }
    let aus = starts.windows(2).map(|w| buf[w[0]..w[1]].to_vec()).collect();
    buf.drain(..starts[starts.len() - 1]);
    aus
}

fn au_has_idr(au: &[u8]) -> bool {
    au.windows(4).any(|w| w[0] == 0 && w[1] == 0 && w[2] == 1 && w[3] & 0x1f == 5)
}

/// Sends this PC's audio state (volume, mute, phone speaker on or off) to phones.
fn publish_deck(rt: &Handle, node: Node, speaking: bool) {
    let (volume, muted) = pc_volume();
    let mut output_devices = vec![];
    if speaking {
        output_devices.push(broche_core::AudioOutputDevice {
            id: "broche:phone_speaker".into(),
            name: "Phone speaker (Broche)".into(),
            is_default: false,
        });
    }
    rt.spawn(async move {
        let _ = node
            .set_deck_state(DeckState { playing: false, volume, muted, mic_muted: None, output_devices })
            .await;
    });
}

/// The PC's output volume (0..=100) and whether it is muted, from WirePlumber, or `pactl`
/// where `wpctl` is missing (the Flatpak runtime has only `pactl`).
pub fn pc_volume() -> (u8, bool) {
    let out = host("wpctl").args(["get-volume", "@DEFAULT_AUDIO_SINK@"]).output();
    let Some(out) = out.ok().filter(|o| o.status.success()) else {
        let text = |args: &[&str]| {
            host("pactl").args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        };
        let volume = text(&["get-sink-volume", "@DEFAULT_SINK@"]).ok().and_then(|t| pactl_percent(&t));
        let muted = text(&["get-sink-mute", "@DEFAULT_SINK@"]).is_ok_and(|t| t.contains("yes"));
        return (volume.unwrap_or(50), muted);
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let vol = text
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .map_or(50, |v| (v * 100.0).round().clamp(0.0, 100.0) as u8);
    (vol, text.contains("MUTED"))
}

/// The first "NN%" in `pactl get-sink-volume` ("Volume: front-left: 32768 /  50% / …").
fn pactl_percent(text: &str) -> Option<u8> {
    let pct = text.split_whitespace().find_map(|w| w.strip_suffix('%')?.parse::<u32>().ok())?;
    Some(pct.min(100) as u8)
}

pub fn set_pc_volume(volume: u8) -> Result<(), String> {
    run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{:.2}", f32::from(volume) / 100.0)])
        .or_else(|_| run("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &format!("{volume}%")]))
}

pub fn set_pc_mute(mute: bool) -> Result<(), String> {
    let on = if mute { "1" } else { "0" };
    run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", on])
        .or_else(|_| run("pactl", &["set-sink-mute", "@DEFAULT_SINK@", on]))
}

/// A command on this PC. In a Flatpak it runs outside the sandbox (`flatpak-spawn --host`):
/// the sandbox has no `loginctl`/`systemctl`, and PipeWire only lets it read device volume.
pub(crate) fn host(program: &str) -> Command {
    if crate::settings::packaging() == crate::settings::Packaging::Flatpak {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.args(["--host", program]);
        cmd
    } else {
        Command::new(program)
    }
}

/// The hardware addresses of this PC's physical network adapters, for Wake-on-LAN: wired ones
/// first (Wi-Fi rarely wakes a PC that is off), virtual ones (bridges, VPNs, Docker) left out.
/// No broadcast addresses: the phone also sends to 255.255.255.255, which reaches the same network.
pub fn wake_macs(sys_class_net: &std::path::Path) -> Vec<String> {
    let Ok(dir) = std::fs::read_dir(sys_class_net) else { return vec![] };
    let mut found: Vec<(bool, String)> = dir
        .filter_map(|e| e.ok().map(|e| e.path()))
        // Only real hardware has a `device` link; ARP type 1 is Ethernet, Wi-Fi included.
        .filter(|p| p.join("device").exists())
        .filter(|p| std::fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "1"))
        .filter_map(|p| {
            let mac = std::fs::read_to_string(p.join("address")).ok()?.trim().to_lowercase();
            (mac.len() == 17 && mac != "00:00:00:00:00:00").then(|| (p.join("wireless").exists(), mac))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, mac)| mac).collect()
}

/// A PNG or JPEG a phone copied, decoded for the clipboard (egui serves it as `image/png`).
/// The bytes come from another device: decoding may not take more than 256 MiB.
fn clipboard_image(bytes: &[u8]) -> Result<eframe::egui::ColorImage, String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 << 20);
    reader.limits(limits);
    let rgba = reader.decode().map_err(|e| e.to_string())?.into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(eframe::egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let status = host(program).args(args).status().map_err(|e| format!("{program}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{program} failed")) }
}

impl Platform for LinuxPlatform {
    fn set_clipboard(&self, text: &str) -> Result<(), String> {
        self.ui.send(Msg::Clipboard(text.to_string())).map_err(|e| e.to_string())
    }

    fn set_clipboard_image(&self, _mime: &str, bytes: &[u8]) -> Result<(), String> {
        let image = clipboard_image(bytes)?;
        self.ui.send(Msg::ClipboardImage(image)).map_err(|e| e.to_string())
    }

    fn open_link(&self, _from: &DeviceId, url: &str) -> Result<(), String> {
        Command::new("xdg-open")
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn power(&self, action: PowerAction) -> Result<(), String> {
        match action {
            PowerAction::Lock => run("loginctl", &["lock-session"]),
            PowerAction::Sleep => run("systemctl", &["suspend"]),
        }
    }

    // Find my PC: a loud, repeating sound until the phone (or the user) stops it.
    fn start_ringing(&self) {
        let mut slot = self.ringing.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let previous = pc_volume();
        let _ = set_pc_mute(false);
        let _ = set_pc_volume(100);
        let sound = [
            "/usr/share/sounds/freedesktop/stereo/phone-incoming-call.oga",
            "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga",
        ]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists());
        let mut cmd = Command::new("ffplay");
        cmd.args(["-nodisp", "-loglevel", "quiet", "-loop", "0"]);
        match sound {
            Some(path) => cmd.arg(path),
            None => cmd.args(["-f", "lavfi", "-i", "sine=frequency=1000:beep_factor=4"]),
        };
        // Own process group, so stopping also ends the `pw-play` loop's current child.
        let quiet = |c: &mut Command| c.stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn();
        let child = quiet(&mut cmd).or_else(|e| match sound {
            // No ffplay (not installed by default on most desktops): loop PipeWire's player.
            Some(path) => quiet(Command::new("sh").args(["-c", "while pw-play \"$0\"; do :; done", path])),
            None => Err(e),
        });
        match child {
            Ok(child) => *slot = Some((child, previous)),
            Err(e) => {
                let _ = set_pc_volume(previous.0);
                let _ = self.ui.send(Msg::Line(format!(
                    "{} {e}",
                    t("Can't play the ring (is ffplay or pw-play installed?):")
                )));
                return;
            }
        }
        // Through D-Bus, not `notify-send`: works the same inside a Flatpak or an AppImage.
        self.rt.spawn(desktop_notify(t("Your phone is looking for this PC."), 2));
        let _ = self.ui.send(Msg::Ringing(true));
    }

    fn stop_ringing(&self) {
        if let Some((mut child, (volume, muted))) = self.ringing.lock().unwrap().take() {
            kill_group(&mut child);
            let _ = set_pc_volume(volume);
            let _ = set_pc_mute(muted);
            let _ = self.ui.send(Msg::Ringing(false));
        }
    }

    fn mirror_sink(&self, _peer: &DeviceId, session: u32) -> Option<Arc<dyn MirrorSink>> {
        if session == MIRROR_SCREEN {
            return Some(self.sink.clone() as Arc<dyn MirrorSink>);
        }
        self.app_sinks.lock().unwrap().get(&session).map(|s| s.clone() as Arc<dyn MirrorSink>)
    }

    fn webcam_sink(&self, _peer: &DeviceId) -> Option<Arc<dyn broche_core::WebcamSink>> {
        Some(self.cam_sink.clone() as Arc<dyn broche_core::WebcamSink>)
    }

    // A phone asked to see this PC's screen.
    fn mirror_requested(&self, peer: &DeviceId, options: &MirrorStart) -> Result<(), String> {
        if options.session != MIRROR_SCREEN {
            return Err("unsupported mirror session on this PC".into());
        }
        self.start_capture(*peer, options)
    }

    fn mirror_stop_requested(&self, _peer: &DeviceId, session: u32) {
        if session == MIRROR_SCREEN {
            self.stop_capture();
        }
    }

    // The phone's touch, scroll and keys on this PC's shared screen.
    fn mirror_input(&self, peer: &DeviceId, session: u32, input: MirrorInput) {
        if session != MIRROR_SCREEN || self.capture.lock().unwrap().is_none() {
            return;
        }
        let allowed = self
            .node
            .get()
            .and_then(|n| n.device_toggles(*peer).ok())
            .is_some_and(|t| t.iter().any(|(k, v)| *k == "remote_input" && *v));
        if !allowed {
            return;
        }
        let cmd = match input {
            MirrorInput::Touch { action, x, y } => Cmd::Touch { action, x, y },
            MirrorInput::Scroll { x, y, dx, dy } => Cmd::Scroll { x, y, dx, dy },
            MirrorInput::Text { text } => Cmd::Input(RemoteInput::Text { text }),
            MirrorInput::Key { key } => {
                let Some((_, name)) = MIRROR_KEYS.iter().find(|(k, _)| *k == key) else { return };
                Cmd::Input(RemoteInput::Key { key: (*name).into(), mods: vec![] })
            }
            MirrorInput::Gamepad { .. } => return,
        };
        let _ = self.cmd.send(cmd);
    }

    fn remote_input(&self, _peer: &DeviceId, input: RemoteInput) -> Result<(), String> {
        // Drawn here, not injected: no desktop portal needed for a dot.
        if let RemoteInput::Laser { on, x, y } = input {
            self.laser.set(on, x, y);
            return Ok(());
        }
        self.cmd.send(Cmd::Input(input)).map_err(|_| "remote input is not available".to_string())
    }

    fn deck_press(&self, _from: &DeviceId, tile: &str) -> Result<(), String> {
        use broche_core::MediaAction;
        let custom = self.deck.read().unwrap().iter().find(|t| t.id == tile).map(|t| t.action.clone());
        if let Some(action) = custom {
            return crate::deck::run(&action);
        }
        match tile {
            "play_pause" | "prev_track" | "next_track" => self.rt.block_on(async {
                let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
                let players = crate::pc_media::players(&conn).await?;
                let player = players.first().ok_or("No active media player")?;
                let action = match tile {
                    "prev_track" => MediaAction::Previous,
                    "next_track" => MediaAction::Next,
                    _ if player.playing => MediaAction::Pause,
                    _ => MediaAction::Play,
                };
                crate::pc_media::command(&player.id, action, None).await
            }),
            "vol_up" | "vol_down" => {
                let (v, _) = pc_volume();
                set_pc_volume(if tile == "vol_up" {
                    v.saturating_add(5).min(100)
                } else {
                    v.saturating_sub(5)
                })?;
                self.publish_audio_state();
                Ok(())
            }
            "vol_mute" => {
                set_pc_mute(!pc_volume().1)?;
                self.publish_audio_state();
                Ok(())
            }
            "lock_pc" => run("loginctl", &["lock-session"]),
            _ => Err("This action is unavailable on Linux".into()),
        }
    }

    fn media_command(
        &self,
        player: &str,
        action: broche_core::MediaAction,
        position: Option<u64>,
    ) -> Result<(), broche_core::MediaError> {
        self.rt
            .block_on(crate::pc_media::command(player, action, position))
            .map_err(broche_core::MediaError::Failed)
    }

    fn set_pc_audio(&self, from: &DeviceId, volume: Option<u8>, muted: Option<bool>) -> Result<(), String> {
        if let Some(level) = volume {
            if level == broche_core::deck::PHONE_SPEAKER_ON {
                // The phone's own sound playing here would come back to it: send that home first.
                if crate::bt_music::current().is_some_and(|p| p.on) && crate::bt_music::claim() {
                    // Not block_on: the tray asks from inside the runtime.
                    self.rt.spawn(crate::bt_music::switch(false, None));
                }
                self.start_speaker(*from)?;
                self.publish_audio_state();
                let _ = self.ui.send(Msg::SpeakerChanged(true));
                return Ok(());
            }
            if level == broche_core::deck::PHONE_SPEAKER_OFF {
                self.stop_speaker();
                self.publish_audio_state();
                let _ = self.ui.send(Msg::SpeakerChanged(false));
                return Ok(());
            }
            set_pc_volume(level)?;
        }
        if let Some(m) = muted {
            set_pc_mute(m)?;
        }
        // The core sends its own state right after this; keep the phone speaker in it.
        self.publish_audio_state();
        let _ = self.ui.send(Msg::AudioChanged);
        Ok(())
    }

    // ---- The folder this PC shares with phones ----

    fn storage_list(&self, path: &str) -> Result<Vec<StorageEntry>, StorageError> {
        self.storage(|s| s.list(path))
    }

    fn storage_open_read(&self, path: &str) -> Result<StorageReadFile, StorageError> {
        self.storage(|s| s.open_read(path))
    }

    fn storage_write(
        &self,
        path: &str,
        staged: &std::path::Path,
        modified: Option<i64>,
    ) -> Result<StorageWriteDone, StorageError> {
        let done = self.storage(|s| s.write(path, staged, modified))?;
        let _ = self.ui.send(Msg::Line(format!("{} {path}", t("Phone saved in the shared folder:"))));
        Ok(done)
    }

    fn storage_mkdir(&self, path: &str) -> Result<(), StorageError> {
        self.storage(|s| s.mkdir(path))
    }

    fn storage_rename(&self, from: &str, to: &str) -> Result<(), StorageError> {
        self.storage(|s| s.rename(from, to))
    }

    fn storage_delete(&self, path: &str, confirmed: bool) -> Result<(), StorageError> {
        self.storage(|s| s.delete(path, confirmed))
    }
}

// ---- Portal session: remote mouse and keyboard, and screen capture ----

struct Injector {
    remote: RemoteDesktop,
    screencast: Screencast,
    session: Session<RemoteDesktop>,
    stream: Option<(u32, (i32, i32))>,
}

async fn open_injector() -> Result<Injector, String> {
    use ashpd::desktop::{
        PersistMode,
        remote_desktop::{SelectDevicesOptions, StartOptions},
        screencast::SelectSourcesOptions,
    };
    let saved = crate::settings::Settings::load();
    let remote = RemoteDesktop::new().await.map_err(|e| e.to_string())?;
    let screencast = Screencast::new().await.map_err(|e| e.to_string())?;
    let session = remote.create_session(Default::default()).await.map_err(|e| e.to_string())?;
    remote
        .select_devices(
            &session,
            SelectDevicesOptions::default()
                .set_devices(DeviceType::Keyboard | DeviceType::Pointer)
                .set_persist_mode(PersistMode::ExplicitlyRevoked)
                .set_restore_token(if saved.portal_token.is_empty() {
                    None
                } else {
                    Some(saved.portal_token.as_str())
                }),
        )
        .await
        .map_err(|e| e.to_string())?;
    screencast
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_sources(ashpd::enumflags2::BitFlags::from_flag(SourceType::Monitor))
                .set_cursor_mode(CursorMode::Embedded)
                .set_multiple(false),
        )
        .await
        .map_err(|e| e.to_string())?;
    let started = remote
        .start(&session, None, StartOptions::default())
        .await
        .map_err(|e| e.to_string())?
        .response()
        .map_err(|e| e.to_string())?;
    if let Some(token) = started.restore_token() {
        crate::settings::Settings::update(|s| s.portal_token = token.to_string());
    }
    let stream = started.streams().first().map(|s| (s.pipe_wire_node_id(), s.size().unwrap_or((1920, 1080))));
    Ok(Injector { remote, screencast, session, stream })
}

/// Unused this long (no input and no screen being sent), the desktop session closes, and with
/// it GNOME's sharing indicator; the next input or capture opens it again, without a prompt
/// while the restore token is valid.
const PORTAL_IDLE: Duration = Duration::from_secs(120);

async fn portal_worker(
    mut rx: mpsc::UnboundedReceiver<Cmd>,
    ui: mpsc::UnboundedSender<Msg>,
    streaming: Arc<Mutex<Option<Capture>>>,
) {
    let mut injector: Option<Injector> = None;
    let mut retry_after = Instant::now();
    loop {
        let cmd = match tokio::time::timeout(PORTAL_IDLE, rx.recv()).await {
            Ok(Some(cmd)) => cmd,
            Ok(None) => break,
            Err(_) => {
                let sending =
                    streaming.lock().unwrap().as_ref().is_some_and(|c| !c.stop.load(Ordering::SeqCst));
                if !sending && let Some(inj) = injector.take() {
                    let _ = inj.session.close().await;
                }
                continue;
            }
        };
        if injector.is_none() {
            if Instant::now() < retry_after {
                if let Cmd::Capture(reply) = cmd {
                    let _ = reply.send(Err("Desktop permission was declined; try again shortly".into()));
                }
                continue;
            }
            match open_injector().await {
                Ok(i) => injector = Some(i),
                Err(e) => {
                    retry_after = Instant::now() + Duration::from_secs(10);
                    crate::settings::Settings::update(|s| s.portal_token.clear());
                    let _ = ui.send(Msg::Line(format!(
                        "{} {e}",
                        t("Remote control / screen sharing unavailable:")
                    )));
                    if let Cmd::Capture(reply) = cmd {
                        let _ = reply.send(Err(e));
                    }
                    continue;
                }
            }
        }
        let Some(inj) = &injector else { continue };
        let result = match cmd {
            Cmd::Capture(reply) => {
                let r = capture(inj).await;
                let failed = r.is_err();
                let _ = reply.send(r);
                if failed { Err("capture failed".to_string()) } else { Ok(()) }
            }
            other => inject(inj, other).await,
        };
        if let Err(e) = result {
            let _ = ui.send(Msg::Line(format!("{} {e}", t("Remote control failed:"))));
            injector = None;
        }
    }
}

async fn capture(inj: &Injector) -> Result<CaptureInfo, String> {
    let (node, size) = inj.stream.ok_or("the desktop did not share a screen")?;
    let fd = inj
        .screencast
        .open_pipe_wire_remote(&inj.session, Default::default())
        .await
        .map_err(|e| e.to_string())?;
    Ok(CaptureInfo { fd, node, size })
}

fn keysym_for_char(c: char) -> i32 {
    let cp = c as u32;
    if cp < 0x100 { cp as i32 } else { (0x0100_0000 + cp) as i32 }
}

/// The phone's mirror keys and the PC key each one presses. "Recents" is Super, which opens
/// the overview on GNOME and KDE.
const MIRROR_KEYS: [(&str, &str); 11] = [
    (mirror_keys::BACK, "escape"),
    (mirror_keys::HOME, "win"),
    (mirror_keys::RECENTS, "win"),
    (mirror_keys::ENTER, "enter"),
    (mirror_keys::BACKSPACE, "backspace"),
    (mirror_keys::DELETE, "delete"),
    (mirror_keys::LEFT, "left"),
    (mirror_keys::RIGHT, "right"),
    (mirror_keys::UP, "up"),
    (mirror_keys::DOWN, "down"),
    (mirror_keys::TAB, "tab"),
];

fn keysym_for_name(name: &str) -> Option<i32> {
    Some(match name {
        "enter" => 0xff0d,
        "backspace" => 0xff08,
        "tab" => 0xff09,
        "escape" => 0xff1b,
        "space" => 0x20,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "home" => 0xff50,
        "end" => 0xff57,
        "page_up" => 0xff55,
        "page_down" => 0xff56,
        "delete" => 0xffff,
        "win" => 0xffeb,
        f if f.starts_with('f') && f.len() > 1 => {
            0xffbd + f[1..].parse::<i32>().ok().filter(|n| (1..=12).contains(n))?
        }
        s if s.chars().count() == 1 => keysym_for_char(s.chars().next()?),
        _ => return None,
    })
}

fn mod_keysym(m: KeyMod) -> i32 {
    match m {
        KeyMod::Ctrl => 0xffe3,
        KeyMod::Alt => 0xffe9,
        KeyMod::Shift => 0xffe1,
        KeyMod::Win => 0xffeb,
    }
}

async fn tap_keys(inj: &Injector, mods: &[i32], key: i32) -> Result<(), String> {
    use ashpd::desktop::remote_desktop::NotifyKeyboardKeysymOptions as O;
    let p = &inj.remote;
    let s = &inj.session;
    for m in mods {
        p.notify_keyboard_keysym(s, *m, KeyState::Pressed, O::default()).await.map_err(|e| e.to_string())?;
    }
    p.notify_keyboard_keysym(s, key, KeyState::Pressed, O::default()).await.map_err(|e| e.to_string())?;
    p.notify_keyboard_keysym(s, key, KeyState::Released, O::default()).await.map_err(|e| e.to_string())?;
    for m in mods.iter().rev() {
        p.notify_keyboard_keysym(s, *m, KeyState::Released, O::default()).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn button(inj: &Injector, code: i32, states: &[KeyState]) -> Result<(), String> {
    use ashpd::desktop::remote_desktop::NotifyPointerButtonOptions;
    for st in states {
        inj.remote
            .notify_pointer_button(&inj.session, code, *st, NotifyPointerButtonOptions::default())
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn move_abs(inj: &Injector, x: f32, y: f32) -> Result<(), String> {
    use ashpd::desktop::remote_desktop::NotifyPointerMotionAbsoluteOptions;
    let (node, (w, h)) = inj.stream.ok_or("no shared screen")?;
    inj.remote
        .notify_pointer_motion_absolute(
            &inj.session,
            node,
            f64::from(x.clamp(0.0, 1.0)) * f64::from(w),
            f64::from(y.clamp(0.0, 1.0)) * f64::from(h),
            NotifyPointerMotionAbsoluteOptions::default(),
        )
        .await
        .map_err(|e| e.to_string())
}

async fn inject(inj: &Injector, cmd: Cmd) -> Result<(), String> {
    use ashpd::desktop::remote_desktop::{NotifyPointerAxisOptions, NotifyPointerMotionOptions};
    let p = &inj.remote;
    let s = &inj.session;
    let input = match cmd {
        Cmd::Input(i) => i,
        Cmd::Touch { action, x, y } => {
            move_abs(inj, x, y).await?;
            return match action {
                TouchAction::Down => button(inj, 0x110, &[KeyState::Pressed]).await,
                TouchAction::Up => button(inj, 0x110, &[KeyState::Released]).await,
                TouchAction::Move => Ok(()),
            };
        }
        Cmd::Scroll { x, y, dx, dy } => {
            move_abs(inj, x, y).await?;
            return p
                .notify_pointer_axis(s, f64::from(dx), f64::from(dy), NotifyPointerAxisOptions::default())
                .await
                .map_err(|e| e.to_string());
        }
        Cmd::Capture(_) => return Ok(()),
    };
    match input {
        RemoteInput::Move { dx, dy } => p
            .notify_pointer_motion(s, f64::from(dx), f64::from(dy), NotifyPointerMotionOptions::default())
            .await
            .map_err(|e| e.to_string()),
        RemoteInput::Button { button: b, action } => {
            let code = match b {
                MouseButton::Left => 0x110,
                MouseButton::Right => 0x111,
                MouseButton::Middle => 0x112,
            };
            let states: &[KeyState] = match action {
                ButtonAction::Down => &[KeyState::Pressed],
                ButtonAction::Up => &[KeyState::Released],
                ButtonAction::Click => &[KeyState::Pressed, KeyState::Released],
            };
            button(inj, code, states).await
        }
        RemoteInput::Scroll { dx, dy } => p
            .notify_pointer_axis(s, f64::from(dx), f64::from(dy), NotifyPointerAxisOptions::default())
            .await
            .map_err(|e| e.to_string()),
        RemoteInput::Text { text } => {
            for c in text.chars() {
                let key = if c == '\n' { 0xff0d } else { keysym_for_char(c) };
                tap_keys(inj, &[], key).await?;
            }
            Ok(())
        }
        RemoteInput::Key { key, mods } => {
            let (mods, key): (Vec<i32>, Option<i32>) = match key.as_str() {
                "copy" => (vec![0xffe3], Some('c' as i32)),
                "paste" => (vec![0xffe3], Some('v' as i32)),
                "cut" => (vec![0xffe3], Some('x' as i32)),
                "undo" => (vec![0xffe3], Some('z' as i32)),
                "select_all" => (vec![0xffe3], Some('a' as i32)),
                "task_view" => (vec![], Some(0xffeb)),
                "lock" => {
                    let _ = run("loginctl", &["lock-session"]);
                    return Ok(());
                }
                other => (mods.iter().map(|m| mod_keysym(*m)).collect(), keysym_for_name(other)),
            };
            match key {
                Some(k) => tap_keys(inj, &mods, k).await,
                None => Ok(()),
            }
        }
        RemoteInput::Slide { action } => {
            let key = match action {
                SlideAction::Next => 0xff53,
                SlideAction::Previous => 0xff51,
                SlideAction::Start => 0xffc2,
                SlideAction::Stop => 0xff1b,
                SlideAction::Black => 'b' as i32,
            };
            tap_keys(inj, &[], key).await
        }
        // Drawn by `crate::laser` in `remote_input`; it never gets here.
        RemoteInput::Laser { .. } => Ok(()),
    }
}

/// A desktop notification (org.freedesktop.Notifications); `urgency` 1 normal, 2 urgent.
/// The connection notifications are sent on. One for the app's lifetime: GNOME removes an
/// app's notifications as soon as the D-Bus connection that sent them closes, so a connection
/// per call lost them.
pub async fn notify_conn() -> Option<&'static zbus::Connection> {
    static CONN: tokio::sync::OnceCell<zbus::Connection> = tokio::sync::OnceCell::const_new();
    CONN.get_or_try_init(zbus::Connection::session).await.ok()
}

pub async fn desktop_notify(body: String, urgency: u8) {
    let Some(conn) = notify_conn().await else { return };
    // Without desktop-entry GNOME can't tell the notification is Broche's and may keep it
    // in the list without showing it (seen on GNOME 50 with the XWayland window).
    let hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> = [
        ("urgency", zbus::zvariant::Value::from(urgency)),
        ("desktop-entry", zbus::zvariant::Value::from("io.github.robertcjla.Broche")),
    ]
    .into();
    let _ = conn
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "Broche",
                0u32,
                "io.github.robertcjla.Broche",
                "Broche",
                body.as_str(),
                Vec::<&str>::new(),
                hints,
                -1i32,
            ),
        )
        .await;
}

/// This PC's battery from `/sys/class/power_supply` (readable inside Flatpak too), if it has one.
pub fn read_pc_battery() -> Option<broche_core::Battery> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    let mut battery = None;
    let mut on_ac = false;
    for entry in dir.flatten() {
        let p = entry.path();
        let read =
            |f: &str| std::fs::read_to_string(p.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
        match read("type").as_str() {
            "Battery" if read("scope") != "Device" && battery.is_none() => {
                let level: u8 = read("capacity").parse().ok()?;
                let status = read("status");
                battery = Some((level.min(100), status == "Charging" || status == "Full"));
            }
            "Mains" | "USB" if read("online") == "1" => on_ac = true,
            _ => {}
        }
    }
    let (level, charging) = battery?;
    Some(broche_core::Battery {
        level,
        charging: charging || on_ac,
        plugged: on_ac.then(|| "ac".to_string()),
        full_in: None,
        cell_bars: None,
        cell_type: None,
        wifi_bars: None,
    })
}

/// Tells paired phones about this PC's battery (their PC card shows it), once a minute
/// or sooner when it changes. Desktops without a battery send nothing.
pub async fn report_pc_battery(node: Node) {
    let mut last: Option<(u8, bool)> = None;
    loop {
        if let Some(b) = read_pc_battery() {
            let now = (b.level, b.charging);
            if last != Some(now) {
                last = Some(now);
                node.update_battery(b).await;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
    }
}

/// Folders a phone must never browse, even when the shared folder is the home folder:
/// keys, passwords, browser profiles, cloud credentials and this app's own identity.
/// Canonical paths, so the check holds through symlinks.
pub fn sensitive_folders() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return vec![] };
    let home = std::fs::canonicalize(&home).unwrap_or(home);
    let mut v: Vec<PathBuf> = [
        ".ssh",
        ".gnupg",
        ".password-store",
        ".pki",
        ".aws",
        ".azure",
        ".kube",
        ".docker",
        ".netrc",
        ".git-credentials",
        ".cargo/credentials",
        ".cargo/credentials.toml",
        ".mozilla",
        ".thunderbird",
        ".local/share/keyrings",
        ".local/share/broche",
        // Older builds' data (identity key, database) may still be on disk: never browsable.
        ".local/share/Nectarlink",
        ".config/nectarlink-linux",
        ".local/share/kwalletd",
        ".config/broche",
        ".config/google-chrome",
        ".config/chromium",
        ".config/BraveSoftware",
        ".config/microsoft-edge",
        ".config/vivaldi",
        ".config/Signal",
        ".config/Element",
        ".config/gh",
        ".config/rclone",
        ".var/app",
    ]
    .iter()
    .map(|p| home.join(p))
    .collect();
    if let Some(d) = dirs::data_local_dir() {
        v.push(d.join("broche"));
        v.push(d.join("Nectarlink"));
    }
    v.into_iter().map(|p| std::fs::canonicalize(&p).unwrap_or(p)).collect()
}

/// Kills a child started with `process_group(0)` and everything it started.
fn kill_group(child: &mut Child) {
    let group = rustix::process::Pid::from_child(child);
    let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    #[test]
    fn wake_macs_are_the_physical_adapters_wired_first() {
        let root = std::env::temp_dir().join(format!("broche-wake-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let add = |name: &str, mac: &str, kind: &str, device: bool, wireless: bool| {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("address"), format!("{mac}\n")).unwrap();
            std::fs::write(dir.join("type"), format!("{kind}\n")).unwrap();
            if device {
                std::fs::create_dir_all(dir.join("device")).unwrap();
            }
            if wireless {
                std::fs::create_dir_all(dir.join("wireless")).unwrap();
            }
        };
        add("wlp1s0", "AA:BB:CC:00:00:02", "1", true, true);
        add("enp2s0", "aa:bb:cc:00:00:01", "1", true, false);
        add("lo", "00:00:00:00:00:00", "772", false, false);
        add("docker0", "02:42:00:00:00:03", "1", false, false);
        add("wwan0", "aa:bb:cc:00:00:04", "65534", true, false);
        assert_eq!(super::wake_macs(&root), ["aa:bb:cc:00:00:01", "aa:bb:cc:00:00:02"]);
        assert!(super::wake_macs(&root.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_copied_image_is_decoded_for_the_clipboard() {
        let mut png = Vec::new();
        image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128])
            .unwrap()
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let img = super::clipboard_image(&png).unwrap();
        assert_eq!(img.size, [2, 1]);
        assert_eq!(img.pixels[0], eframe::egui::Color32::from_rgba_unmultiplied(255, 0, 0, 255));
        assert_eq!(img.pixels[1], eframe::egui::Color32::from_rgba_unmultiplied(0, 0, 255, 128));
        assert!(super::clipboard_image(b"not an image").is_err());
    }

    #[test]
    fn pactl_volume_is_read() {
        let out = "Volume: front-left: 32768 /  50% / -18.06 dB,   front-right: 32768 /  50% / -18.06 dB\n";
        assert_eq!(super::pactl_percent(out), Some(50));
        assert_eq!(super::pactl_percent("Volume: mono: 98304 / 150% / 10.57 dB"), Some(100));
        assert_eq!(super::pactl_percent("Connection failure"), None);
    }

    use super::*;

    #[test]
    fn every_mirror_key_presses_a_real_key() {
        for (key, name) in MIRROR_KEYS {
            assert!(keysym_for_name(name).is_some(), "{key} → {name} has no key code");
        }
    }

    #[test]
    fn stopping_the_ring_also_ends_the_loops_current_player() {
        // `sleep` stands in for the `pw-play` that is playing when the ring stops.
        let mut child =
            Command::new("sh").args(["-c", "while sleep 30; do :; done"]).process_group(0).spawn().unwrap();
        let group = child.id().to_string();
        std::thread::sleep(Duration::from_millis(200));
        kill_group(&mut child);
        let left = Command::new("pgrep").args(["-g", &group]).output().unwrap();
        assert!(left.stdout.is_empty(), "left running: {}", String::from_utf8_lossy(&left.stdout));
    }

    #[test]
    fn splits_access_units() {
        let mut b = vec![];
        b.extend_from_slice(&[0, 0, 0, 1, 9, 0x10, 0, 0, 0, 1, 7, 1, 0, 0, 0, 1, 5, 9]);
        b.extend_from_slice(&[0, 0, 0, 1, 9, 0x30, 0, 0, 1, 1, 7]);
        let p = aud_positions(&b);
        assert_eq!(p, vec![0, 18]);
        assert!(au_has_idr(&b[p[0]..p[1]]));
        assert!(!au_has_idr(&b[p[1]..]));
    }

    #[test]
    fn every_complete_access_unit_is_sent() {
        let au = |t: u8| vec![0, 0, 0, 1, 9, 0x10, 0, 0, 0, 1, t, 0xAA];
        let mut buf = [au(5), au(1), au(1)].concat();
        // Two complete units (the third may still grow), none lost.
        let got = take_access_units(&mut buf);
        assert_eq!(got, vec![au(5), au(1)]);
        assert_eq!(buf, au(1));
        // One complete unit once the next one starts.
        buf.extend_from_slice(&au(1));
        assert_eq!(take_access_units(&mut buf), vec![au(1)]);
        assert_eq!(buf, au(1));
    }

    #[test]
    fn target_size_fits_and_aligns() {
        assert_eq!(target_size((2560, 1440), 1280), (1280, 720));
        let (w, h) = target_size((1366, 768), 1920);
        assert_eq!((w % 16, h % 16), (0, 0));
    }

    #[test]
    fn every_encoder_gets_all_its_numbers() {
        for (name, _, template) in H264_ENCODERS {
            let filled = fill_encoder(template, 6000, 30);
            assert!(filled.starts_with(name) && !filled.contains('{'), "{filled}");
        }
        assert!(fill_encoder(H264_ENCODERS[0].2, 6000, 30).contains("bitrate=6000000"));
    }

    #[test]
    fn keysyms() {
        assert_eq!(keysym_for_name("f5"), Some(0xffc2));
        assert_eq!(keysym_for_name("enter"), Some(0xff0d));
        assert_eq!(keysym_for_name("a"), Some('a' as i32));
        assert_eq!(keysym_for_name("f13"), None);
    }
}
