// SPDX-License-Identifier: GPL-3.0-or-later
//! Shows the phone's screen or camera: the H.264 stream goes through `ffmpeg`,
//! which hands back JPEG frames for the window; the phone's sound goes to `ffplay`.
//!
//! Counters (packets, keyframes, decoded frames, decoder errors) tell whether a frozen
//! picture comes from the phone (few packets or keyframes) or from the decoder (errors): the
//! window asks for a fresh keyframe, and `BROCHE_STREAM_LOG=1` logs them. Set `BROCHE_DUMP_DIR` to also
//! write every received stream to a `.h264` file for offline analysis.

use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Sender, SyncSender, channel, sync_channel},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use broche_core::{MirrorAudioConfig, MirrorConfig, MirrorSink, WebcamConfig, WebcamSink};
use eframe::egui;

/// Packets waiting for the decoder, at most. Beyond this it is behind: drop
/// until the next keyframe instead of freezing further and further.
const QUEUE: usize = 90;

/// What the window shows; shared with the threads that fill it.
#[derive(Default)]
pub struct MirrorShared {
    pub frame: Mutex<Option<egui::ColorImage>>,
    pub size: Mutex<(u32, u32)>,
    pub active: AtomicBool,
    pub ctx: Mutex<Option<egui::Context>>,
    /// The window that shows it, when not the main one (a phone app's window).
    pub viewport: Mutex<Option<egui::ViewportId>>,
    pub packets: AtomicU64,
    pub keyframes: AtomicU64,
    pub bytes: AtomicU64,
    pub decoded: AtomicU64,
    pub errors: AtomicU64,
    pub dropped: AtomicU64,
    /// Decoded pictures replaced by a newer one before the window drew them.
    pub replaced: AtomicU64,
    /// Longest wait between two packets from the network.
    pub packet_gap: Gap,
    /// Longest wait between two decoded pictures.
    pub picture_gap: Gap,
    /// When someone last had the picture in front of them (drawn, window not minimized).
    pub seen: Mutex<Option<std::time::Instant>>,
    /// Counts the times the screen was asked for: a watch for an older one stops watching.
    pub run: AtomicU64,
    /// The main window's X11 id (0 until known), to tell when it is minimized.
    pub window: std::sync::atomic::AtomicU32,
}

/// The longest time between two events since it was last read: a smooth average can hide
/// a picture that stood still for a moment.
#[derive(Default)]
pub struct Gap {
    last: Mutex<Option<std::time::Instant>>,
    longest_ms: AtomicU64,
}

impl Gap {
    pub fn tick(&self) {
        let now = std::time::Instant::now();
        if let Some(before) = self.last.lock().unwrap().replace(now) {
            self.longest_ms.fetch_max(now.duration_since(before).as_millis() as u64, Ordering::Relaxed);
        }
    }

    /// The longest gap since the last call, in milliseconds.
    pub fn take(&self) -> u64 {
        self.longest_ms.swap(0, Ordering::Relaxed)
    }
}

impl std::fmt::Debug for MirrorShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MirrorShared")
    }
}

/// A snapshot of the counters, to show rates.
#[derive(Clone, Copy, Default)]
pub struct Counters {
    pub packets: u64,
    pub keyframes: u64,
    pub bytes: u64,
    pub decoded: u64,
    pub errors: u64,
    pub dropped: u64,
    pub replaced: u64,
}

impl MirrorShared {
    fn repaint(&self) {
        if let Some(ctx) = self.ctx.lock().unwrap().as_ref() {
            match *self.viewport.lock().unwrap() {
                Some(id) => ctx.request_repaint_of(id),
                None => ctx.request_repaint(),
            }
        }
    }

    pub fn counters(&self) -> Counters {
        Counters {
            packets: self.packets.load(Ordering::Relaxed),
            keyframes: self.keyframes.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            decoded: self.decoded.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            replaced: self.replaced.load(Ordering::Relaxed),
        }
    }

    fn reset(&self) {
        for c in [&self.packets, &self.keyframes, &self.bytes, &self.decoded, &self.errors, &self.dropped] {
            c.store(0, Ordering::Relaxed);
        }
    }
}

struct Decoder {
    child: Child,
    input: SyncSender<Vec<u8>>,
    /// False once ffmpeg has exited (it is then started again on the next packet).
    alive: Arc<AtomicBool>,
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Sink {
    label: &'static str,
    shared: Arc<MirrorShared>,
    decoder: Mutex<Option<Decoder>>,
    size: Mutex<(u32, u32)>,
    codec: Mutex<String>,
    /// After a drop or a decoder error, only a keyframe can restart the picture.
    waiting_key: AtomicBool,
    dump: Mutex<Option<std::fs::File>>,
    audio: Mutex<Option<(Child, Sender<Vec<u8>>)>>,
}

impl std::fmt::Debug for Sink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sink")
    }
}

/// Reads one binary PPM (`P6 <w> <h> 255`, then RGB bytes) from ffmpeg.
fn read_ppm(r: &mut impl Read) -> std::io::Result<(usize, usize, Vec<u8>)> {
    // The header: four whitespace-separated fields, the last followed by one whitespace byte.
    let mut fields: Vec<String> = Vec::with_capacity(4);
    let mut cur = String::new();
    let mut byte = [0u8; 1];
    while fields.len() < 4 {
        r.read_exact(&mut byte)?;
        let c = byte[0];
        if c.is_ascii_whitespace() {
            if !cur.is_empty() {
                fields.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c as char);
            if cur.len() > 16 {
                return Err(std::io::Error::other("not a PPM frame"));
            }
        }
    }
    if fields[0] != "P6" || fields[3] != "255" {
        return Err(std::io::Error::other("not an 8-bit PPM frame"));
    }
    let w: usize = fields[1].parse().map_err(std::io::Error::other)?;
    let h: usize = fields[2].parse().map_err(std::io::Error::other)?;
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return Err(std::io::Error::other("bad frame size"));
    }
    let mut rgb = vec![0u8; w * h * 3];
    r.read_exact(&mut rgb)?;
    Ok((w, h, rgb))
}

/// The ffmpeg arguments that turn an H.264 (or H.265) stream into raw RGB frames.
///
/// Raw frames instead of JPEG: on this kind of stream (no timestamps) the JPEG encoder
/// stops ffmpeg after a few frames ("Invalid pts"), which froze the picture; and
/// encoding then decoding a JPEG per frame costs more than copying the pixels.
/// Pictures larger than 1280 px are scaled down: nobody sees more in a window.
pub fn decoder_args(codec: &str) -> Vec<String> {
    let demuxer = if matches!(codec, "h265" | "hevc") { "hevc" } else { "h264" };
    [
        "-loglevel",
        "error",
        "-probesize",
        "32",
        "-analyzeduration",
        "0",
        // Frame threading (FFmpeg's own H.264 decoder, as in the Flatpak) holds back one frame per
        // core: 9 frames, ~300 ms, on this PC; a still screen showed a tap only after more frames came.
        "-flags",
        "low_delay",
        "-f",
        demuxer,
        "-i",
        "pipe:0",
        "-an",
        "-vf",
        "scale='if(gte(iw,ih),min(1920,iw),-2)':'if(gte(iw,ih),-2,min(1920,ih))'",
        "-fps_mode",
        "passthrough",
        "-flush_packets",
        "1",
        "-f",
        "image2pipe",
        "-c:v",
        "ppm",
        "pipe:1",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl Sink {
    pub fn new(label: &'static str, shared: Arc<MirrorShared>) -> Arc<Self> {
        Arc::new(Self {
            label,
            shared,
            decoder: Mutex::new(None),
            size: Mutex::new((0, 0)),
            codec: Mutex::new("h264".into()),
            waiting_key: AtomicBool::new(false),
            dump: Mutex::new(None),
            audio: Mutex::new(None),
        })
    }

    fn open_dump(&self) {
        let Some(dir) = std::env::var_os("BROCHE_DUMP_DIR") else { return };
        let _ = std::fs::create_dir_all(&dir);
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let path = std::path::Path::new(&dir).join(format!("{}-{ts}.h264", self.label));
        *self.dump.lock().unwrap() = std::fs::File::create(path).ok();
    }

    fn start_decoder(&self) -> Result<Decoder, std::io::Error> {
        let codec = self.codec.lock().unwrap().clone();
        let mut child = Command::new("ffmpeg")
            .args(decoder_args(&codec))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let alive = Arc::new(AtomicBool::new(true));
        let (tx, rx) = sync_channel::<Vec<u8>>(QUEUE);
        std::thread::spawn(move || {
            while let Ok(chunk) = rx.recv() {
                if stdin.write_all(&chunk).is_err() {
                    break;
                }
            }
        });
        // Each line ffmpeg complains about is one decoder error.
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    shared.errors.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        let shared = self.shared.clone();
        let alive2 = alive.clone();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::with_capacity(1 << 20, stdout);
            while let Ok((w, h, rgb)) = read_ppm(&mut reader) {
                *shared.size.lock().unwrap() = (w as u32, h as u32);
                if shared.frame.lock().unwrap().replace(egui::ColorImage::from_rgb([w, h], &rgb)).is_some() {
                    shared.replaced.fetch_add(1, Ordering::Relaxed);
                }
                shared.picture_gap.tick();
                shared.decoded.fetch_add(1, Ordering::Relaxed);
                shared.repaint();
            }
            alive2.store(false, Ordering::SeqCst);
        });
        Ok(Decoder { child, input: tx, alive })
    }

    fn config_stream(&self, codec: &str, width: u32, height: u32) {
        let mut slot = self.decoder.lock().unwrap();
        // The phone may repeat its config: only start over when the picture changed.
        let mut size = self.size.lock().unwrap();
        let same_codec = *self.codec.lock().unwrap() == codec;
        if slot.is_some() && *size == (width, height) && same_codec {
            return;
        }
        *size = (width, height);
        *self.codec.lock().unwrap() = codec.to_string();
        *slot = None;
        self.shared.reset();
        self.waiting_key.store(true, Ordering::SeqCst);
        self.open_dump();
        match self.start_decoder() {
            Ok(d) => *slot = Some(d),
            Err(e) => eprintln!("can't start ffmpeg: {e}"),
        }
        self.shared.active.store(true, Ordering::Release);
    }

    fn packet_in(&self, keyframe: bool, data: Vec<u8>) {
        self.shared.packets.fetch_add(1, Ordering::Relaxed);
        self.shared.packet_gap.tick();
        self.shared.bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
        if keyframe {
            self.shared.keyframes.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(f) = self.dump.lock().unwrap().as_mut() {
            let _ = f.write_all(&data);
        }
        let mut slot = self.decoder.lock().unwrap();
        // ffmpeg stopped (it shouldn't, but a frozen picture is the worst outcome):
        // start it again; it needs a keyframe, which the window asks the phone for.
        if slot.as_ref().is_some_and(|d| !d.alive.load(Ordering::SeqCst)) {
            *slot = None;
            self.shared.errors.fetch_add(1, Ordering::Relaxed);
            if let Ok(d) = self.start_decoder() {
                *slot = Some(d);
            }
            self.waiting_key.store(true, Ordering::SeqCst);
        }
        // After a gap the picture can only restart on a keyframe.
        if self.waiting_key.load(Ordering::SeqCst) {
            if !keyframe {
                self.shared.dropped.fetch_add(1, Ordering::Relaxed);
                return;
            }
            self.waiting_key.store(false, Ordering::SeqCst);
        }
        if let Some(d) = slot.as_ref()
            && d.input.try_send(data).is_err()
        {
            // The decoder is behind: skip to the next keyframe rather than freeze.
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
            self.waiting_key.store(true, Ordering::SeqCst);
        }
    }

    fn stream_ended(&self) {
        *self.decoder.lock().unwrap() = None;
        *self.size.lock().unwrap() = (0, 0);
        *self.dump.lock().unwrap() = None;
        self.shared.active.store(false, Ordering::Release);
        *self.shared.frame.lock().unwrap() = None;
        self.shared.repaint();
    }
}

impl MirrorSink for Sink {
    fn config(&self, config: MirrorConfig) {
        self.config_stream(&config.codec, config.width, config.height);
    }

    fn packet(&self, keyframe: bool, _time_us: u64, data: Vec<u8>) {
        self.packet_in(keyframe, data);
    }

    fn ended(&self) {
        *self.audio.lock().unwrap() = None;
        self.stream_ended();
    }

    fn audio_config(&self, config: MirrorAudioConfig) {
        let child = play_pcm(config.rate, config.channels);
        if let Ok(mut child) = child {
            let mut stdin = child.stdin.take().expect("piped");
            let (tx, rx) = channel::<Vec<u8>>();
            std::thread::spawn(move || {
                while let Ok(chunk) = rx.recv() {
                    if stdin.write_all(&chunk).is_err() {
                        break;
                    }
                }
            });
            *self.audio.lock().unwrap() = Some((child, tx));
        }
    }

    fn audio(&self, _time_us: u64, data: Vec<u8>) {
        if let Some((_, tx)) = self.audio.lock().unwrap().as_ref() {
            let _ = tx.send(data);
        }
    }

    fn audio_ended(&self) {
        if let Some((mut child, _)) = self.audio.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
}

/// Plays raw 16-bit PCM from stdin: PipeWire's `pw-play` (every modern desktop has it), or
/// `ffplay` where it is missing.
fn play_pcm(rate: u32, channels: u8) -> std::io::Result<Child> {
    let spawn = |program: &str, args: Vec<String>| {
        Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    };
    let pw = ["--raw", "--format=s16", &format!("--rate={rate}"), &format!("--channels={channels}"), "-"];
    spawn("pw-play", pw.map(String::from).to_vec())
        .or_else(|_| spawn("ffplay", ffplay_pcm_args(rate, channels)))
}

/// `-ch_layout`, not `-ac`: FFmpeg 7+ refuses `-ac` on raw input, and ffplay then quit at once,
/// so the phone's sound never played.
fn ffplay_pcm_args(rate: u32, channels: u8) -> Vec<String> {
    let rate = rate.to_string();
    let layout = format!("{channels}c");
    [
        "-nodisp",
        "-loglevel",
        "quiet",
        "-fflags",
        "nobuffer",
        "-f",
        "s16le",
        "-ar",
        &rate,
        "-ch_layout",
        &layout,
    ]
    .into_iter()
    .chain(["-i", "pipe:0"])
    .map(String::from)
    .collect()
}

/// The phone's camera as a webcam: the same H.264 pipeline as the screen.
pub struct CamSink(pub Arc<Sink>);

impl std::fmt::Debug for CamSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CamSink")
    }
}

impl WebcamSink for CamSink {
    fn config(&self, config: WebcamConfig) {
        self.0.config_stream(&config.codec, config.width, config.height);
    }

    fn packet(&self, keyframe: bool, _time_us: u64, data: Vec<u8>) {
        self.0.packet_in(keyframe, data);
    }

    fn ended(&self) {
        self.0.stream_ended();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ppm_frames_back_to_back() {
        let mut data = b"P6\n2 1\n255\n".to_vec();
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        data.extend_from_slice(b"P6 1 1 255\n");
        data.extend_from_slice(&[7, 8, 9]);
        let mut r = std::io::Cursor::new(data);
        assert_eq!(read_ppm(&mut r).unwrap(), (2, 1, vec![1, 2, 3, 4, 5, 6]));
        assert_eq!(read_ppm(&mut r).unwrap(), (1, 1, vec![7, 8, 9]));
        assert!(read_ppm(&mut r).is_err());
    }

    #[test]
    fn phone_sound_plays_with_current_ffmpeg() {
        let args = ffplay_pcm_args(48_000, 2);
        assert!(args.windows(2).any(|w| w == ["-ch_layout", "2c"]));
        assert!(!args.iter().any(|a| a == "-ac"));
    }

    #[test]
    fn decoder_args_pick_the_codec() {
        assert!(decoder_args("h264").windows(2).any(|w| w == ["-f", "h264"]));
        assert!(decoder_args("h265").windows(2).any(|w| w == ["-f", "hevc"]));
        assert!(decoder_args("h264").windows(2).any(|w| w == ["-flags", "low_delay"]));
    }
}
