# Broche for Linux

Independent modified build based on NectarLink 0.1.4 by Bee/thtbee and contributors,
without affiliation or endorsement. Original licenses and notices are retained in LICENSE,
LICENSES/ and TRADEMARKS.md.

A graphical app for Linux built on the shared Rust core (`broche-core`).
It keeps one node online for the whole session, so the phone sees one steady link. Only one
Broche runs per data folder: opening it again shows the window that is already open.

## What it does

- **Home**: the phone's state, quick actions (find my phone, music), drop files to send,
  send text or a link, the phone's quick settings (volume, ringer, Do Not Disturb…), this PC's volume,
  and, with the phone also paired in the system's Bluetooth settings, a switch that plays the
  phone's sound on this PC over Bluetooth (also in the tray menu).
- **Messages and calls**: SMS, call log, dialling, answering, contacts. With the phone also
  paired in the system's Bluetooth settings and PipeWire's telephony service
  (`org.pipewire.Telephony`), calls are answered and hung up through Bluetooth and their audio
  goes through the PC (Settings › Preferences › Bluetooth calls).
- **Notifications**: see, reply to and dismiss the phone's notifications, optionally also as
  desktop notifications.
- **Screen and camera**: the phone's screen (with touch) and camera in a window, and phone apps
  in windows of their own (advanced mode on the phone). The camera shows only in Broche (save a
  photo or scan a document from it); it isn't a camera device other apps can use. The phone can also see this PC's screen,
  control its mouse and keyboard (through the desktop portals), and use it as a speaker.
- **Files and photos**: where received files go (`~/Broche/Recibidos`), the phone's storage and
  its gallery, `~/Broche` (or the folder chosen for the phone to browse), and History.
- **Settings**: Preferences (this PC's name, Bluetooth calls, appearance, window and island,
  language, PC shortcuts on the phone, startup and closing, files, clipboard, notifications, video
  quality), Device (the connection check and permissions per phone), Pair,
  and About (version and missing programs).

Outside the window:

- **Island**: a pill at the top of the screen for calls, transfers (with progress) and music.
  It grows when a call rings and hides when nothing is going on.
- **Shelf**: a panel at the screen's edge with the phone's latest photo, screenshot, clip and
  files. Files dropped on it go to the phone; received ones drag out into other apps.
- **Laser pointer**: the phone's pointer in presentation mode, drawn over the screen.
- **Command palette** (Ctrl+K): jump to a page or run an action by typing a few letters.
- **PC shortcuts**: your own buttons for the phone's shortcuts page (open an app or a link, run
  a command), kept in `deck.json` next to the settings. The phone only learns their names.

## Desktop integration

Video quality is Normal (720p, 30 fps) or High (1080p, 60 fps, experimental), in Preferences;
it is kept the same on the phone, so it can be changed on either device.

The StatusNotifier tray keeps the connection alive after closing the window. Click the icon to
restore it; Settings › Quit application exits completely. Its menu shows the phone's state and
offers find my phone, its screen, its files, sending files, Do Not Disturb, phone as speaker,
the shelf, the received and shared folders, and Settings. GNOME needs an AppIndicator host;
without one the window closes normally, so it cannot become inaccessible.

The window always runs through XWayland (the island, shelf and laser need X11 to stay on top);
pure Wayland is only a fallback.

The phone learns this PC's network adapter addresses (wired first) when it may use PC actions,
so its **Wake** button can wake the PC with Wake-on-LAN (enable it in the BIOS and for the
network card; Wi-Fi rarely wakes a PC that is off).

PC players are detected through MPRIS, without installing a media-controller utility.
Files are selected through the system portal. The shared folder defaults to `~/Broche`, not the
whole Home folder, and sensitive folders inside it are never shown; access is allowed separately
for each paired device. The `storage` permission controls browsing, and `remote_files`
additionally permits access over a relay outside the LAN.

## Build and run

```sh
cargo build --release -p broche-gui -p broche-cli
target/release/broche
```

Needs Rust (see `rust-toolchain.toml`) and a C toolchain. At run time it uses these programs when present:

| Feature | Program (Fedora package) |
|---|---|
| Phone screen/camera view | `ffmpeg` (ffmpeg-free is enough) |
| Phone sound | `pw-play` (pipewire-utils), else `ffplay` |
| Share this PC's screen | `gst-launch-1.0` with `openh264enc` and `pipewiresrc` (`gstreamer1-plugin-openh264`, `pipewire-gstreamer`) |
| PC volume | `wpctl` (wireplumber), else `pactl` |
| Phone as speaker | `pw-record` (pipewire-utils), else `parec` |
| Find my PC ring | `ffplay`, else `pw-play` |
| Links, folders | `xdg-open` (xdg-utils) |

**About** lists the missing ones and the command that installs them on your distribution.
Remote mouse/keyboard and PC screen sharing go through the desktop's RemoteDesktop/ScreenCast portal
(GNOME/KDE on Wayland ask the user once).

## Diagnostics

| Variable | Effect |
|---|---|
| `BROCHE_DATA_DIR=<dir>` | Another identity and paired-devices folder |
| `BROCHE_CONFIG_DIR=<dir>` | Another preferences folder |
| `BROCHE_LOG=<filter>` | Log level of the core (default `warn,zbus=error`) |
| `BROCHE_DUMP_DIR=<dir>` | Also write each received screen/camera stream to `<dir>/*.h264` |
| `BROCHE_STREAM_LOG=1` | Log the phone screen/camera counters every second |
| `BROCHE_SHOT=<dir>` | Save a picture of every page and quit (`BROCHE_SHOT_SIZE`, `BROCHE_SHOT_PAGES`, `BROCHE_SHOT_WAIT`, `BROCHE_TEST_STORAGE=1`) |
| `BROCHE_WELCOME=<page>` | Open the welcome at that page (1–5) |
| `BROCHE_AUTOSTART=screen\|camera\|shelf\|app:<package>` | Start that as soon as the phone connects |
| `BROCHE_TEST_SCREEN=1` | Share a test picture instead of this PC's screen |

[docs/technical/building-and-testing.md](../../docs/technical/building-and-testing.md) has the
details and a simulated phone for tests.

## Packaging

```sh
scripts/package-linux.sh           # target/package/Broche-<ver>-linux-<arch>.tar.gz (+ .AppImage)
COMPAT=1 scripts/package-linux.sh  # …-compat.tar.gz, built in Ubuntu 22.04 with podman: runs on glibc 2.35+
./install.sh --prefix ~/.local     # from the tarball; --uninstall removes it
```

A normal build needs the glibc of the machine that built it (2.43 on Fedora 44). The `COMPAT=1`
build runs on Ubuntu 22.04, Debian 12, Fedora 36 and later. Checked in a clean Ubuntu 22.04: it
installs and `broche-cli` runs. The window opens once these (usually already installed) libraries
are there: `libx11-6 libx11-xcb1 libxcursor1 libxrandr2 libxi6 libxkbcommon-x11-0 libgl1 libegl1`.
The first `COMPAT=1` build downloads the Ubuntu image and Rust into podman volumes
(`broche-cargo`, `broche-rustup`). Later builds reuse them.

The app id is `io.github.robertcjla.Broche`; `data/` holds the `.desktop`, AppStream metadata and icons.

- **Tarball / distro packages**: ready (use `COMPAT=1` for other distributions). They depend on
  the programs in the table above.
- **AppImage**: `scripts/package-linux.sh` also makes an AppImage when `appimagetool` is in the
  `PATH` (1.9.1 checked); it never downloads it. A `runtime-<arch>` file next to it, or
  `APPIMAGE_RUNTIME=<file>`, saves downloading the AppImage runtime on every build. The tool's
  output goes to `target/package/appimagetool.log`. Checked on Fedora 44 and in a clean Ubuntu
  22.04 (`APPIMAGE_EXTRACT_AND_RUN=1`, no FUSE there). Like the tarball, it uses the host's
  graphics libraries and helper programs. About lists the missing ones, and autostart points
  at the AppImage file (`$APPIMAGE`).
- **Flatpak** (the main format): `packaging/flatpak/io.github.robertcjla.Broche.yml`, on the
  Freedesktop 26.08 runtime, which already has every helper: ffmpeg/ffplay, GStreamer with
  `pipewiresrc` and the VA/openh264 encoders, `pw-play`/`pw-record`/`parec`, and `pactl`
  (Broche uses it for the PC volume when `wpctl` is missing).

  ```sh
  flatpak install --user flathub org.freedesktop.Platform//26.08 org.freedesktop.Sdk//26.08 \
    org.freedesktop.Sdk.Extension.rust-stable//26.08
  scripts/flatpak-cargo-sources.py Cargo.lock > apps/linux/packaging/flatpak/cargo-sources.json
  flatpak-builder --user --install --force-clean --state-dir=target/flatpak/state \
    target/flatpak/build apps/linux/packaging/flatpak/io.github.robertcjla.Broche.yml
  ```

  The Flatpak talks to `org.freedesktop.Flatpak` to run the PC volume (`wpctl`/`pactl`), lock
  (`loginctl`) and sleep (`systemctl`) on the host: inside the sandbox PipeWire allows only reading
  the volume. This permission lets the app run commands outside the sandbox. It may also talk to
  `org.pipewire.Telephony` (session bus) and BlueZ (system bus) for Bluetooth calls and the
  phone's sound over Bluetooth, and to the accessibility bus for screen readers.

  Regenerate `cargo-sources.json` whenever `Cargo.lock` changes. Checked with a real phone:
  pairing, screen, sound, volume, clipboard, files, tray, notifications, Bluetooth calls and
  autostart (through the Background portal). The Flatpak keeps its own identity and settings
  (`~/.var/app/io.github.robertcjla.Broche`), so the phone links it as a new PC. Better not to
  install it next to the native build: both add a "Broche" entry with the same ID to the menu.
  Their autostart entries are separate (`io.github.robertcjla.Broche.desktop` for the Flatpak,
  `broche.desktop` for the native build and the AppImage).
