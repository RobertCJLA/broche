# Architecture

Broche is built around NectarLink's shared Rust core (`broche-core`): peer discovery, cryptographic identity, QUIC sessions, capability negotiation, persistent storage, and media/file framing. Two shells bind to it: the Linux desktop app (`apps/linux`, Rust + egui, calling the core directly) and the Android app (`apps/android/app`, Kotlin, through UniFFI).

## System Topology

```text
Linux PC: apps/linux (broche) ─┐   broche-cli (same data folder, not at the same time)
  egui UI, tray (SNI), portals (ashpd), PipeWire, ffmpeg/GStreamer, MPRIS
  └─ broche-core + broche-protocol (+ broche-adb for the USB tunnel)
            │ LAN QUIC (UDP + mDNS) · USB ADB tunnel · relay (away_mode only)
Android: apps/android/app
  Compose UI, ConnectionService, NotificationListener, Camera2
  ├─ broche-ffi (UniFFI → libbroche_ffi.so) embedding the core
  └─ advanced-mode helper (app_process as shell, loopback TCP)
```

## Workspace Crates and Components

### 1. `crates/broche-protocol`

Pure wire-protocol crate with no runtime IO or platform dependencies beyond `tokio::io::{AsyncRead, AsyncWrite}` for stream framing.

- **Envelope and Framing (`src/frame.rs`, `src/envelope.rs`)**: unsigned LEB128 length-prefixed CBOR `Envelope` encoding (`ciborium`), with a 1 MiB frame ceiling (`MAX_FRAME_LEN = 1 << 20`).
- **Messages (`src/messages.rs`)**: every message type and its body, one module per service (`types`, `files`, `storage`, `mirror`, `calls`, `deck`…). Capability IDs (`core.ping`, `files.transfer`, `mirror.view`…) are listed in [docs/protocol/capabilities.md](../protocol/capabilities.md).
- **Errors (`src/error.rs`)**: `ErrorCode` (`UNSUPPORTED`, `UNPAIRED`, `BAD_MESSAGE`, `DENIED`, `BUSY`, `NOT_FOUND`…) and their QUIC close codes.
- **Pairing Primitives (`src/pairing.rs`)**: the `broche://pair` URI (`PairingUri`), `ALPN_PAIR` (`"broche-pair/0"`), `ALPN_SESSION` (`"broche/0"`), the QR proof (`pair_proof`, HMAC-SHA256) and the 6-digit code (`sas_code`).
- **Video Packetizer (`src/video.rs`)**: real-time video packets (`PacketKind`: config, frame, keyframe) with microsecond timestamps and an 8 MiB packet limit (`MAX_VIDEO_PACKET`).

### 2. `crates/broche-core`

The central networking, USB transport, persistence, and feature runtime; one module per feature (`sms.rs`, `calls.rs`, `storage.rs`, `mirror.rs`, `webcam.rs`, `deck.rs`…).

- **Node and Session Engine (`src/node.rs`, `src/session.rs`)**: manages the `iroh::Endpoint`, accepts `ALPN_PAIR` and `ALPN_SESSION` connections, runs the `hello` / `hello.update` capability handshake, keeps per-peer connection state, and multiplexes control messages, bulk streams and QUIC datagrams.
- **USB Tunnel (`src/usb.rs`, with `crates/broche-adb`)**: `broche-adb` speaks the ADB host protocol directly over TCP (`127.0.0.1:5037`, `host:track-devices`) without running `adb` for every poll; `usb.rs` forwards a port to the phone (`DEFAULT_USB_PORT = 53319`) and carries QUIC datagrams over that TCP link (`NLQU` header), still encrypted end to end. A USB-connected phone with USB debugging on is also paired automatically.
- **Persistence (`src/store.rs`)**: embedded SQLite database (`broche.db`, 9 migrations) storing paired devices, per-device toggles, key-value settings, the timeline, notification conversations and the persistent send queue ([Storage and Retention](storage-and-retention.md)).
- **Resumable Transfers (`src/transfer.rs`)**: multi-file transfers with BLAKE3 verification, `.part` resume, pause/resume/cancel, and queueing while a peer is offline.
- **Capability Matrix (`src/features.rs`)**: `FEATURES`, `DEVICE_TOGGLES` and permission profiles; `compute` turns capabilities, power level and toggles into feature states.
- **Album-art Palette (`src/palette.rs`)**: Material-style light and dark palettes (`ThemeScheme`) from album covers.
- **Encrypted Clipboard History (`src/clipboard_history.rs`)**: recent text and image clips in `clipboard_history.enc` and `clipboard_images/<id>.enc`, sealed with the platform `KeyProtector` (the Linux desktop keyring or the Android Keystore).
- **Interoperability (`src/localsend.rs`, `src/handoff.rs`)**: LocalSend v2 discovery and file transfer (port 53317), and handoff helpers (map and video links with positions, documents opened on arrival).

### 3. `crates/broche-ffi`

UniFFI `0.32` bridge (`src/lib.rs`) compiled via `cargo-ndk` into `libbroche_ffi.so` (by default for `arm64-v8a` and `x86_64`).

- Wraps `broche_core::Node` in the `BrocheNode` object; plain records and enums cross the boundary ([core API](../architecture/core-api.md)).
- Foreign traits implemented in Kotlin: `EventListener` (node events), `Platform` (what the phone does when a PC asks) and `KeyProtector`, so Android's `KeystoreKeyProtector` (AES-256-GCM inside `AndroidKeyStore`, hardware-backed when the device supports it) seals `identity.key` and `clipboard_history.enc` inside Rust.

### 4. `apps/cli` (`broche-cli`)

Command-line client for headless use, scripted tests and diagnostics (`id`, `pair`, `join`, `devices`, `send`, `calls`, `demo`, `incoming-call`…). It can act as a phone (`--as-phone`) to simulate one.

### 5. `apps/linux` (`broche`)

The Linux desktop app: egui/eframe UI (`main.rs`, `shell.rs`, `kit.rs`, `theme.rs`, `welcome.rs`, `palette.rs` for Ctrl+K), StatusNotifierItem tray (`tray.rs`), MPRIS players (`pc_media.rs`), the platform side of every feature (`platform.rs`: input and screen capture through XDG portals, PipeWire audio, volume, find-my-PC, lock/suspend, shared folder), phone screen/camera decoding through `ffmpeg` (`mirror.rs`), windows outside the main one (`island.rs`, `shelf.rs` with its own XDND drag in `xdnd.rs`, `laser.rs`, `apps.rs`), Bluetooth calls (`bt_calls.rs`) and the phone's sound over Bluetooth (`bt_music.rs`, A2DP through BlueZ), desktop notifications (`phone_notify.rs`), the X11 clipboard watcher (`clipwatch.rs`), translations (`i18n.rs`) and settings/autostart (`settings.rs`). The device key is sealed by the desktop keyring (`secret-service` feature). The app runs through XWayland.

### 6. `apps/android/app` (`io.github.robertcjla.broche`)

The Android application written in Kotlin with Jetpack Compose (`compileSdk = 37`, `targetSdk = 37`, `minSdk = 26`). [apps/android/README.md](../../apps/android/README.md) maps its packages.

- **Foreground Service (`service/ConnectionService.kt`)**: keeps the node reachable (with the Wi-Fi multicast lock mDNS needs); `core/Core.kt` owns the `BrocheNode` and folds its events into `CoreState`.
- **Notification Listener (`notifications/NotificationListener.kt`, `NotificationReader.kt`)**: mirrors active notifications, `MessagingStyle` conversations, inline replies and actions.
- **Feature handlers**: one per package (`calls/PhoneCalls.kt`, `sms/PhoneSms.kt`, `contacts/PhoneContacts.kt`, `media/PhoneMedia.kt`, `photos/RecentPhotos.kt`, `storage/PhoneStorage.kt`, `toggles/PhoneToggles.kt`, `mirror/MirrorService.kt`, `webcam/WebcamService.kt`, `audio/PcSpeakerPlayer.kt`).
- **Advanced-mode Helper (`elevated/Elevated.kt`, `elevated/InputServer.kt`, `elevated/AppDisplay.kt`)**: an optional helper process launched with `app_process` (`InputServer <token>`) as `shell` (UID 2000), through the phone's own wireless debugging or root. It binds a loopback TCP socket (`127.0.0.1:<ephemeral_port>`), requires a 24-byte (48-hex-char) per-session `SecureRandom` token on the first line (checked in constant time with `MessageDigest.isEqual`, `ElevatedHandshake.kt`), injects touch, keys and scrolling, uses `/dev/uhid` for virtual keyboards and gamepads when it can, and creates virtual displays (`AppDisplay.kt`) for phone apps in windows of their own.

## Discovery and Transport Architecture

Broche uses `iroh` (`1.3`) to run a single QUIC + TLS 1.3 endpoint per device. Every paired connection is classified into one of three `ConnectionPath` variants (`crates/broche-core/src/node.rs`):

| Path | Discovery Mechanism | Transport Path | Latency & Bandwidth Profile |
|---|---|---|---|
| `ConnectionPath::Lan` | mDNS (`iroh-mdns-address-lookup`, service name `"broche"`) + cached direct socket addresses from `peers.last_addrs` | Direct UDP QUIC between LAN IPv4/IPv6 sockets | Lowest latency, full Wi-Fi / Ethernet throughput |
| `ConnectionPath::Usb` | `crates/broche-adb` device tracker (`127.0.0.1:5037`) forwarding to `127.0.0.1:53319` (`DEFAULT_USB_PORT`) | QUIC UDP datagrams framed over TCP (`NLQU` header in `src/usb.rs`) via `adb forward` | Zero Wi-Fi dependency, deterministic wired latency |
| `ConnectionPath::Relay` | Opt-in **Connect over mobile data** (`away_mode = true`) enabling Pkarr/DNS address lookup and iroh's default relays (`RelayMode::Default`) | End-to-end TLS 1.3 encrypted QUIC relayed via `iroh` relay servers | Works across cellular and different networks; subject to relay capability guards |

### QUIC Transport Parameters

Configured in `crates/broche-core/src/node.rs`:

- `KEEP_ALIVE_INTERVAL`: `1,000 ms` (1 second)
- `IDLE_TIMEOUT`: `8s` (detects network drops and triggers path failover within seconds)
- `STREAM_WINDOW`: `16 MiB` per stream
- `CONNECTION_WINDOW`: `32 MiB` per connection

### Automatic Path Failover and Deduping

When both USB and Wi-Fi are available, or when a phone transitions between Wi-Fi and a **Connect over mobile data** (`away_mode`) relay path, both peers may attempt simultaneous connections. `Node` resolves concurrent dials deterministically and updates the active session handle in place so upper-layer features (the timeline, the send queue, media, notifications) continue operating against the peer's `DeviceId`.

## Album-art Palette

`crates/broche-core/src/palette.rs` derives light and dark UI palettes (`ThemeScheme`) from album artwork, so the music card and the island take the colours of what plays (`extract_artwork_scheme_rgb`, `extract_artwork_scheme_rgba`):

1. **Downsample** (`downsample_rgba` / `downsample_rgb`): average blocks down to at most `MAX_SAMPLE_SIDE = 112` pixels a side.
2. **Trim borders** (`trim_borders`): drop near-black, near-white or flat letterbox strips so they don't decide the colour.
3. **Pick a seed** (`seed_from_grid`, `seed_from_pixels`): quantize the remaining pixels (`QuantizerCelebi`, up to `QUANTIZE_COLORS = 128` colours), score hue families by population, chroma and tone, and average the winner in OKLab. Grey covers get the neutral `ACHROMATIC_SEED_RGB` (`achromatic_scheme`).
4. **Build the scheme** (`scheme_from_seed_rgb`): four tonal palettes and the 15 roles in `ROLES`, for light and dark, with readable contrast (tested in `extreme_phone_seeds_keep_calm_surfaces_and_wcag_contrast`). Results are cached (`CACHE_CAPACITY = 64`).

## Bluetooth call audio on Linux

The GUI uses the session-bus service `org.pipewire.Telephony` for HFP calls. Its root
ObjectManager lists audio gateways; each gateway has a separate ObjectManager for calls.
The watcher subscribes before enumeration and refreshes after signals, with a periodic
refresh for service recovery. One connected gateway is selected automatically; with multiple
gateways, `bt_phone` selects its address. Multiple calls are matched by digits in the number;
ambiguous matches keep the existing network call control.

Answer and Hang up in the island and Calls use HFP when a call matches. Other controls and
unmatched calls keep the existing protocol. The paired device's `calls` permission also gates
Bluetooth controls. A failed Bluetooth action is reported without retrying it over the network.
Answer activates the gateway transport unless `bt_talk_on_answer=false`; an already active
transport is left alone. Audio travels through PipeWire, never through the Broche protocol.
Returning audio to the phone uses its call screen. BlueZ on the system bus supplies aliases;
Bluetooth pairing remains in system settings. Flatpak permits these two D-Bus services only.
