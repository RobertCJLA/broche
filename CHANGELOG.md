# Changelog

All notable changes to Broche, an independent modified build of NectarLink.
Versions follow [semantic versioning](https://semver.org); before 1.0, a minor
version may change things that need a step from you, and its notes say so.
How versions, tags and this file are maintained: [docs/VERSIONING.md](docs/VERSIONING.md).

Broche starts from NectarLink 0.1.4; NectarLink's own history is in
[its repository](https://github.com/thtbee/nectarlink/blob/main/CHANGELOG.md).

## Unreleased

## 0.2.1 (2026-11-11)

- Android builds no longer embed the build machine's Rust source paths.
- Update the bundled SQLite and Rust system libraries to address security advisories.
- LocalSend rejects oversized chunk headers and keeps its previous setting if saving a change fails.
- The file browser reports when a folder is too large to list instead of hiding entries.

## 0.2.0 (2026-11-11)

Broche's first release: a **Linux desktop app** on NectarLink's shared core, and the Android
app adapted, in English and Spanish. **Beta.** Broche and NectarLink can't talk to each other
(the protocol is `broche/0`), so install Broche on both the PC and the phone.

- **Linux**: Flatpak (recommended), AppImage and a tarball for glibc 2.35 and later. Tray menu,
  welcome and first setup, command palette (Ctrl+K), History, connection check, the phone's
  screen in a window of its own with its sound, phone apps in their own windows, a shelf and an
  island at the edges of the screen, the laser pointer, automatic clipboard (images from the
  phone too), album art both ways, Catppuccin and Tokyo Night themes, screen reader names checked
  with Orca. Pairing over a USB cable, phone calls over Bluetooth (answer, decline and talk
  through the PC), the phone's sound on the PC over Bluetooth, and waking the PC from the phone
  (Wake-on-LAN).
- **Android**: a simpler Home / Activity / Settings layout with a search in Settings, a language
  picker, fewer permissions, permission profiles when pairing, device names, a redone
  home-screen widget, and "Copy to PC clipboard" in the share menu.
- **Security**: the PC's device key in the desktop keyring, LocalSend HTTPS-only with the
  certificate checked, dependency updates.
- **Removed**: Windows, the auto-updater, the pause for banking apps, Simplified Chinese, the
  phone's brightness control from the PC, the voice recorder, the desk clock and "Wake when PC
  is nearby".

Every change follows, newest first.

- **Linux, Android**: About has a link to Broche's code on GitHub (with its known problems and
  releases).
- **Linux**: Home has **Stop sharing this PC's screen** while a phone shows it; before, only the
  phone or GNOME's indicator could stop it. The technical line under the phone's screen and
  camera ("fps shown · decoder errors…", English only) is gone from the window; it is still
  logged with `BROCHE_STREAM_LOG=1`. The native build and the AppImage keep their autostart
  entry in `~/.config/autostart/broche.desktop`, not under the app ID the Flatpak uses, so
  switching autostart in one no longer switches the other; `install.sh --uninstall` removes it.
- **Linux**: **Phone's sound on this PC (Bluetooth)**, under the phone's music on Home, in the
  tray menu and in the command palette (Ctrl+K), sends the phone's music and sound to this PC over Bluetooth, as to a speaker, and
  back. It needs the phone paired with the PC in the system's Bluetooth settings; Broche only
  connects or releases that one profile (A2DP), never calls or the pairing, and plays the music
  again after switching, since Android pauses it. It turns off "Phone as speaker" (and that one
  turns it off) so the sound doesn't loop between the two, waits while a switch is under way, and
  says why when the phone is still connecting or the PC can't take Bluetooth sound. Without
  Bluetooth on the PC it checks only every 30 seconds and never starts the Bluetooth service.
- **Release scripts**: `scripts/bump-version.sh 0.2.0` now works for this first release. The
  version was set to 0.2.0 ahead of time, so the script refused ("already at 0.2.0"); given the
  current version and no tag for it, it folds the Unreleased notes into that version's section
  and dates it (and the AppStream release) today.
  `scripts/release-local.sh` checks its tools and the APK's Rust targets before the long builds, so a
  missing one stops it at once instead of after the Flatpak.
- **Linux**: the PC tells the phone its network adapters' hardware addresses (wired first), so
  **Wake** on the phone (Home card and widget) can wake it with Wake-on-LAN. It shares them only
  with phones allowed PC actions, as before.
- **Android**: "Wake when PC is nearby" (Companion Device) is gone. It waited for a Bluetooth
  beacon only NectarLink's Windows app sent, so it never worked with Broche, and it asked for
  three background permissions (`REQUEST_COMPANION_RUN_IN_BACKGROUND`,
  `REQUEST_COMPANION_START_FOREGROUND_SERVICES_FROM_BACKGROUND`,
  `REQUEST_OBSERVE_COMPANION_DEVICE_PRESENCE`), which the app no longer requests. `BLUETOOTH_SCAN`,
  declared but never asked for, is gone too.
- **Linux, Android, docs**: a pre-release review of what both apps say. The About pages say
  Broche isn't an official NectarLink release or endorsed by its author. The PC's tips cover the
  island, shelf, dropping files, copying images from the phone, USB without Wi-Fi, the connection
  check, History and desktop notifications, and no longer call the phone's camera a webcam: it
  shows only in Broche, and video-call apps can't use it (also said on the Camera page, in the
  permission texts and in the phone's guide). Permission texts now mention images on the
  clipboard, photos and scans under Photos, and the PC's volume under PC actions; the phone's
  call, clipboard, sharing and advanced-mode texts match what works. The README gains a "What it
  doesn't do" section.
- **Core, CLI, protocol**: NectarLink's voice recorder is gone. No Broche app recorded or saved
  voice notes, but the core, the FFI, the CLI (`broche-cli record`) and `files.offer`
  (`recording`, `markers`) still carried it, and the CLI announced `recorder`. The database
  moves to schema 9: queued recordings are dropped, their history entries stay as files, and the
  `recordings` per-device toggle is forgotten. `docs/protocol/recorder.md` is removed.
- **Android**: a second share entry, **Copy to PC clipboard**, for images: the picture lands on
  the PC's clipboard, ready to paste, instead of arriving as a file. The window every share
  shows now follows Broche's theme, with a preview (the picture, file name or count, or the
  text), the PC it goes to and whether it's connected, and a PC choice when there are several.
- **Android**: while advanced mode runs, Settings has two switches, off by default: **Control
  calls from your PC** (mute, speaker, hold and keypad on the call in progress) and **Sensitive
  notifications on your PC** (Android 15+; includes one-time codes). Turning one on grants that
  permission through advanced mode; turning it off returns it to Android's default. Starting
  advanced mode still never grants them. The advanced-mode steps no longer claim that
  one-time codes reach the PC.
- **Linux**: images copied on the phone (the "Copy to PC" button or tile, or the "Copy to PC
  clipboard" share entry) now land on the PC's clipboard as PNG, ready to paste in any app.
  Before, the PC refused them with "this device has no clipboard".
- **Release script**: `scripts/release-local.sh` now copies the `-compat` tarball and AppImage
  that its `COMPAT=1` build makes; before, it copied files with the plain names, which that
  build never writes (so it failed or shipped an older, non-portable build).
- **Docs**: the README, both app READMEs, the technical docs and the protocol specs were checked
  against the code and brought up to date: the island, shelf, Bluetooth calls, phone apps in
  windows, USB and offline start; how the Linux app (not NectarLink's Windows app) handles
  clipboard, volume, keyboard, music and storage; the real database schema, capability IDs,
  error codes and pairing details; no in-app updater. The voice recorder and `storage.mount`
  are marked as not offered by Broche's apps.
- **Linux**: during a Bluetooth call the window and the island no longer read the settings file on
  every frame to find the phone chosen for calls; it is kept in memory and updated when changed.
- **Linux**: answering a Bluetooth call no longer reports a failure when the call was answered but its
  audio could not move to the PC (use "Talk on this PC"). Bluetooth calling is shown as unavailable
  again after the telephony service goes away.
- **Linux, Android**: the Bluetooth call setup is now explained step by step. Settings › Preferences
  (and the welcome, on the permissions page) shows whether the phone is connected and the three
  steps: allow Phone, Call log and Contacts on the phone, pair it in the system Bluetooth settings,
  and keep "Phone calls" on. Broche only explains; it never changes Bluetooth or phone settings.
  The phone's call-audio card says the same, laid out full width with its button under the text.
- **Linux**: with a single Bluetooth call, answering or declining from the phone now checks the
  caller's number, so a call on another phone is not picked up by mistake. Unused Bluetooth call
  code was removed.
- **Core (Linux, Android)**: Broche now starts with no network (flight mode, Wi-Fi still
  connecting). LAN discovery used to stop the whole core from starting, so nothing worked, not
  even over USB, until Broche was restarted. It now starts and looks for the local network every
  few seconds until there is one.
- **Linux**: the transfer panel no longer says "1 files": the count shows only for several files,
  and a single failed transfer or file is named in the singular. Quitting no longer logs a false
  "island shape unavailable" error.

- **Android**: refresh the PC widget when an online connection changes between USB, wireless
  and relay without triggering updates for every latency fluctuation.
- **Linux**: the island shows byte-weighted transfer progress, including queued uploads and
  direct phone downloads. Keep completed bytes in the current batch, show terminal outcomes,
  and ignore late upload-start reports after a locally observed failure.
- **Linux**: keep island text and progress within their bounds, preserve the mouse grab while
  repositioning, and let fullscreen windows cover non-urgent activity. Incoming ringing calls
  retain priority. Screenshot mode does not reach a real phone over USB; it still finds the
  simulated phone on the local network.

- **Linux**: show file-transfer progress in the main window across all pages, including Send,
  dropped files, photos and uploads from Files. Keep completed, cancelled and failed results
  until dismissed, preserve active transfers when dismissing results, and show a failure count.
- **Core**: storage uploads now report throttled transfer progress and terminal outcomes,
  including interrupted uploads and their resumed byte offsets, without protocol changes.

- **Linux**: Files now shows aggregate progress when downloading selected phone files:
  percentage, bytes, completed file count and the active filename. Keep the final summary
  with per-file errors until dismissed or another batch starts. Downloads stay sequential.

- **Android**: use the caller name from a single active call notification when telephony
  provides neither a number nor a name, including late notification updates. Preserve known
  caller identities and leave ambiguous notifications unidentified.

- **Android**: document Bluetooth pairing and call audio requirements in the Calls feature
  guide; clarify that audio stays on the phone when no compatible Bluetooth connection exists.

- **Linux**: answering an already active Bluetooth call no longer reports PipeWire's
  `InvalidState` error or prevents the following audio step, including when the phone answers
  between checking its state and sending Answer.
- **Linux**: add the PipeWire Bluetooth call client and saved phone selection, with isolated
  D-Bus tests. Answer and Hang up in the island and Calls use Bluetooth when a call matches;
  otherwise they keep the existing phone control. Bluetooth errors are reported without
  retrying the action over the network. Answering uses PC audio automatically (configurable);
  active calls also offer Talk on this PC. Bluetooth preferences show the connected phone and
  select one when several are connected; Flatpak includes the required service permissions.
- **Android**: Calls and contacts settings now link to system Bluetooth settings to connect
  the phone to the PC for call audio, without requesting new Bluetooth permissions.

- **Linux, Android**: stopping the PC's ringing locally no longer silences a phone that is
  ringing at the same time. A separate notification clears the phone's Stop ringing action;
  both apps need the update for that notification (older apps safely ignore it).
- **Linux**: only one Broche runs per data folder. Opening it again (from the launcher while it
  sits in the tray, say) shows the window of the one already running. Two at once shared one
  identity and kept taking the phone's connection from each other, most visibly over USB.
- **Linux, Android**: the database and data folder left from before the rename are deleted on
  start; nothing read them and "Clear history" could not reach them.
- **Linux**: the automatic stop of the phone's screen only starts counting once the phone has
  accepted, and no longer leaves the Screen page waiting forever; downloading several files from
  the phone goes one at a time, so large selections no longer time out or overwrite each other.
- **Android**: with two calls ringing, answering from the PC no longer presses the wrong call's
  notification; a nearly square app window no longer asks the encoder for more pixels than it
  started with; the widget's play/pause goes by its own PC's player.
- **Linux**: the old NectarLink data and settings folders stay off-limits to the phone again.
- **Android**: a History title with a huge "(+N more)" no longer crashes the app; the widget asks
  for enough height to show every row.
- **Linux**: the phone's screen also stops 30 s after closing Broche to the tray with ×, not only
  after minimizing it.
- **Android**: a switch for sending what you copy on the phone to the PC on its own (Settings ›
  Features › Clipboard). Before, advanced mode always sent it and no switch stopped it; the PC's
  switch only covers what you copy on the PC, and now says so.
- **Android**: the widget always shows its music row ("Nothing playing" when silent), so its
  layout doesn't jump.
- **Android**: the home-screen widget is redone: Broche's colors (the wallpaper's on Android 12+,
  light or dark), how the PC is connected, what it plays with artwork and previous / play-pause /
  next, and Send files, Copy to PC, PC shortcuts and Control PC. Every text is translated.
- **Linux**: pairing over a USB cable works: plug in the phone with USB debugging on (and adb
  running on the PC) and both show the 6-digit code; Broche comes forward on Settings › Pair.
  Before, the Linux app never watched adb, so nothing happened.
- **Linux**: screen readers (Orca) see the Flatpak: it may now ask the session bus whether
  accessibility is on (`org.a11y.Bus`), which egui needs before it shows anything to them.
- **Linux and Android**: when you stop the PC's ringing on the PC, the phone that started it
  hears about it and no longer offers "Stop ringing".
- **Android**: PC shortcuts show their built-in tiles and the Main and Custom pages in the
  phone's language, not the PC's.
- **Android and Linux**: History titles written by the core ("Copied text: …", "Files",
  "a.txt (+2 more)", "Phone webcam"…) show in the app's language.
- **Linux**: the phone's screen, its apps and its camera show with much less delay in the
  Flatpak: its H.264 decoder no longer holds back a frame per CPU core (9 frames measured).
- **Linux**: High video quality now asks the phone for 1080p at 12 Mbit/s instead of 1280 pixels
  at 6 Mbit/s.
- **Linux**: the phone's own screen (as usual, keep on, off) can be changed while it is shown, and
  "phone only" mode has a button to turn it off or on.
- **Linux**: download several files from the phone at once: click to pick, double-click to
  download one.
- **Linux**: Phone apps is now a tab of Screen and camera (no longer in the side bar), with a
  Home tip; its grid fills the width.
- **Linux**: Home shows how the phone is connected (and the delay) and its model and Android
  version, under "Connected".
- **Linux**: bigger music buttons in the island, with play/pause in a circle; drag the island
  anywhere on the screen and it stays there; Preferences › Window puts it in a corner, back at
  the top centre, or locks it.
- **Android**: phone apps in PC windows are drawn at full resolution and laid out as on the
  phone, instead of in a few pixels with giant text.
- **Android**: answering, declining and hanging up from the PC work for other apps' calls
  (WhatsApp and the like), through the buttons of their call notification.
- **Linux**: the island's places are worked out from the main screen's real size; a phone app
  window follows a new size (maximized, say) even when the app shows nothing new.
- **Linux**: the phone's screen stops by itself 30 seconds after Broche is minimized, or after
  3 minutes on another page, unless its sound is on.
- **Linux**: removed "Join with a pairing link"; phones pair by scanning the PC's code.
- **Linux**: the permission profile's explanation is inside its card.
- **Android**: Home's "Send clipboard" is now "Copy to PC", the same text size as its neighbours.
- **Android**: download several files from the PC at once: touch and hold a file to pick more.

### Stored data under Broche's own names: pair again

- The database is `broche.db`, the Android Keystore key `broche.identity.v1` and the phone's
  storage preferences `broche_storage_saf`. The code that moved folders and data from
  NectarLink's and earlier Broche builds' names is gone. **A build from before this one starts
  as a new device: pair the PC and the phone again.** The Android release is signed with a new
  key, so a test build installed earlier must be uninstalled first.

### Linux

- Pages that need a phone show how to pair one when none is paired, instead of "No paired
  devices. Select a paired phone first."
- On a narrow window the phone picker goes under the page's title instead of covering it.
- Recent calls: declined calls are named (they showed "rejected"), the length reads 4:14 instead of
  254 s and is left out for unanswered calls, and each call says when it was.
- "1 days ago" reads "Yesterday".
- Hints that sent you to Settings › Permissions point to Settings › Device, where permissions are.
- Screen: the "Also play the phone's sound" switch sits next to its text.
- A tip on Home says that Ctrl+K opens the command palette; nothing in the app mentioned it.
- Home: the phone's music has a card of its own, like Android's media widget: the album art large
  on the left, the title and artist, and Previous, a large round Play/Pause and Next. With nothing
  playing it is one quiet line with a Play button.
- Island: it stays a while after the activity stops (Preferences › Window: at once, 5 s, 15 s,
  30 s or 1 min; 15 s by default), so paused music keeps its play button. Calls still go at once.
- Island: a button that fails (a call the phone refuses, a player that doesn't answer) now says
  why on the island for a few seconds and in the status bar; before, nothing happened.
- The phone's notifications show on this PC too (Preferences › Notifications, on by default): the
  ones that popped up on the phone, as "App · title" with the text and the app's icon. An update
  replaces its notification, one removed on the phone goes from the PC, and clicking one opens
  Broche's Notifications page. Silent and ongoing ones (music, navigation) stay on the phone.
- Fixed: the shelf's ▶ (Open) on a received file did nothing: it went through the folder opener,
  which first tried to create a folder with the file's name. It is now an "Open" button, and
  clicking the file opens it too.
- Fixed: in the Flatpak, PC shortcuts › Open an app only offered Broche (the sandbox holds no other
  apps). The list now comes from the system, and apps and commands run outside the sandbox
  (`flatpak-spawn --host`), as they do in the native app.
- Island (Preferences › Window, off by default): a pill at the top centre of the screen, above
  other windows, while there is a call (Answer/Decline, then Mute, Speaker and Hang up), a transfer
  (name and progress) or music on the phone or this PC (album art, Previous/Pause/Next). It grows
  when the pointer is on it or a call rings, hides when nothing is going on, and never takes the
  focus.
- Shelf: a panel at the right edge of the screen, above other windows, with the phone's latest
  photo and screenshot (Save brings them to Recibidos), its latest clip (Copy) and the files it
  sent lately. Drag a received file into another app (X11 drag and drop, written for Broche since
  winit only receives drops; a small page follows the pointer meanwhile), or drop files on the shelf
  to send them to the phone. Open it from
  the tray, Ctrl+K or Home; Escape or ✖ closes it.
- Phone apps: a new Apps section lists the phone's apps (with a search) and opens one in a window
  of its own on the PC, with the mouse, scrolling and keyboard on it (Escape is Back). Resizing the
  window resizes the app's display on the phone, closing it stops the app's stream, and windows keep
  playing while Broche is minimized. It needs advanced mode on the phone. Broche now draws without
  vsync: presenting a frame of a minimized window held up every other window.
- Fixed: a quick click on the phone's screen could leave the finger down (press and release in
  one frame), so the tap never happened.
- Album art: the phone's music on Home shows its cover, and the PC sends the cover of what it plays
  (from the player's `mpris:artUrl`: a file, or downloaded once per track when it is a web address)
  so the phone shows it too. In the Flatpak, covers browsers leave in the host's /tmp are read
  through `flatpak-spawn --host`.
- Command palette: Ctrl+K anywhere opens a search for pages, settings and actions (find my phone,
  send files, check the connection, show the pairing code); arrow keys move, Enter opens, Esc closes.
  It understands other words too ("sonar" finds Find my phone), in English and Spanish.
- Fixed: the clear button of search fields showed an empty box (a glyph missing from the font).
- Connection check (Settings › Device): whether the core runs, the PC is on a local network, a VPN
  is on, the firewall (firewalld) lets the phone in, and each phone's state (connected, through the
  internet relay, on the network but not connecting, not seen), worst first. When the fix needs the
  administrator it shows the command to run; Broche itself never asks for a password.
- History in Linux (Files › History): what moved between this PC and its phones (files, clips, links,
  photos, screen sessions), newest first, with a search, filters by kind, "Show more", and Open for
  links and files that are on this PC. It refreshes when the core records something new.
- The phone's laser pointer (Control PC › presentation) shows on the PC: a red dot with a white rim
  on the primary screen, above everything and letting clicks through; it hides when the phone lets
  go or stops sending. Before, the PC ignored it.
- New look, last step: the remaining opaque panes (phone files, the conversation list and the
  conversation) are glass too (`kit::glass`, shared with the cards).
- New look, third step: motion. The rail's highlight glides to the section you pick, pages fade
  in, and hover highlights fade instead of switching. With reduced motion on in the desktop
  (GNOME: Accessibility › Reduce animation) nothing moves or fades, egui's own animations included.
- New look, second step: the navigation is always the narrow icon rail (as in the original
  NectarLink), with a badge for unread notifications; the phone is picked in the top bar. Home's
  phone card shows the phone drawn, with its battery as a ring on the screen. The "Compact view"
  preference is gone (the rail is always compact).
- New look, first step: glass. Broche paints a soft backdrop tinted with the scheme's colours, and
  the navigation and cards are translucent glass with a light rim and a soft shadow. Catppuccin and
  Tokyo Night stay; the contrast test now also measures text on the glass over the backdrop.
- Fixed: the software store opened the original NectarLink project as Broche's page. The AppStream
  metadata now points to Broche's repository and issues, names the developer and says Broche is an
  independent build of NectarLink.
- Fixed: About › Technical log stayed empty. The core's warnings and errors now reach it (and the
  system log); `BROCHE_LOG` changes the level. An empty log says so.
- The technical log no longer fills with harmless D-Bus warnings from `zbus` ("Failed to populate
  properties cache"); its errors still show.
- The main README is in English.

### Android

- Laser pointer: choose Finger or Tilt above the laser pad (remembered; Finger by default). Before,
  both moved the dot at once and the tilt kept pulling it back to the centre.
- Removed the voice recorder: it could no longer be opened since Home lost its Record button, and
  the guide still advertised it. Its screen, service, saved-recordings list, permission toggle,
  guide entry and the `FOREGROUND_SERVICE_MICROPHONE` permission are gone. The PC no longer
  announces that it takes recordings. Older recordings in History can still be sent again as files.
- Language picker in Settings › Appearance (System / English / Español), on Android 13 and
  later; Broche also appears in the system's per-app language settings. Older Android versions
  keep following the system language.
- English and Spanish copy reviewed: American spelling, sentence case for the Home section titles,
  "Features" instead of "Features & Data" (it overlapped Data & storage), "History" everywhere,
  a real plural for dropped queue items, and the guide's "where to find it" paths updated to the
  current screens (Activity, Settings › Data & storage).
- The last "Timeline"/"Cronología" texts (Data & storage, history search, the guide) say History;
  the guide sends you to Activity › History, and to `~/Broche/Recibidos` on the PC.
- Fixed: "seen 5 minutes ago" and other relative times followed the system language instead of the
  app's.
- Fixed: after "Ring PC", leaving Home (for example to Control PC) and coming back showed
  "Ring PC" again while the PC kept ringing; it now stays "Stop ringing".

### Repository

- Folders reorganized for readability: the apps are in `apps/` (`apps/linux`, the desktop app
  and its packaging, before `desktop/linux-gui`; `apps/android`; `apps/cli`, before
  `core/broche-cli`) and the shared Rust libraries in `crates/` (before `core/`). Build commands
  change accordingly (for example `cd apps/android`); crate and binary names don't.
- The README has a "What Broche adds and changes" section (added, changed, removed) compared
  with NectarLink 0.1.4.
- Removed the maintainer's working notes and AI agent files (AGENTS.md, CLAUDE.md, `.codex/`,
  plans, hand-off and branding notes) and the Android security probe app. The Linux debugging
  helpers and the simulated phone are now in `docs/technical/building-and-testing.md`.

### Linux: Flatpak

- The Flatpak manifest now builds and runs, on the Freedesktop 26.08 runtime. It needs no extra
  modules, because the runtime has ffmpeg, GStreamer, the PipeWire tools and `pactl`.
- `scripts/flatpak-cargo-sources.py` writes the offline Cargo sources (`cargo-sources.json`)
  from `Cargo.lock`, with only Python's standard library.
- The PC volume falls back to `pactl` where `wpctl` is missing.
- Fixed: in the Flatpak, "Start with my session", file pickers and other portal requests hung
  without an answer. Reading the device key opened the portals' shared D-Bus connection on a
  short-lived runtime; that runtime now lives as long as the app.
- Fixed: started with the session, Broche could open without connecting, because the keyring and
  the portals were still starting. It now retries for up to 30 seconds, and each failure is written
  to the system log.
- Fixed: the Flatpak ran on Wayland instead of XWayland, so the automatic clipboard and raising
  the window did not work. `--socket=fallback-x11` removed X11 whenever Wayland was there, overriding
  `--socket=x11`.
- Fixed: in the Flatpak the phone could not change the PC volume (PipeWire lets a Flatpak only read
  device volume), and Lock PC and Sleep failed (no `loginctl`/`systemctl` in the sandbox). They now
  run on the host through `flatpak-spawn --host`, which needs `--talk-name=org.freedesktop.Flatpak`:
  a broad permission that lets the app run commands outside the sandbox.
- In a Flatpak the tray item's bus name is under the app ID, because a Flatpak can't own
  `org.kde.StatusNotifierItem-*`.
- "Start with my session" no longer freezes the window in a Flatpak: the Background portal
  is asked on the app's runtime.
- The phone's screen while it is shown on the PC is one choice (As usual / Keep it on /
  Turn it off) instead of two switches that contradicted each other.

### Linux: AppImage

- `COMPAT=1 scripts/package-linux.sh` also builds `Broche-<ver>-linux-<arch>-compat.AppImage` when
  `appimagetool` is installed. It was checked on Fedora 44 and in a clean Ubuntu 22.04.
- The script uses a saved AppImage runtime when there is one (`APPIMAGE_RUNTIME`, or
  `runtime-<arch>` next to `appimagetool`). If the build fails, it now stops and points to
  `target/package/appimagetool.log` instead of hiding the error.

### Linux: a tarball for older distributions

- `COMPAT=1 scripts/package-linux.sh` builds inside Ubuntu 22.04 with podman. The result,
  `Broche-<ver>-linux-<arch>-compat.tar.gz`, needs glibc 2.35 instead of the build machine's.
  It was checked in a clean Ubuntu 22.04 container, with the window opening and the welcome
  shown.

### Linux: "Follow the system" follows light/dark changes

- With the theme on "Follow the system", Broche switches between light and dark at once when
  the desktop does. Under XWayland the window never heard of the change; now the app asks the
  Settings portal (`color-scheme`) and listens to it.

### Android: guide texts match the app

- The feature guide and the setup walkthrough say "advanced mode", as Settings does, instead of
  "Elevated".
- Wake PC no longer gives Windows steps (Device Manager); it says to turn Wake-on-LAN on in
  the PC's network settings.
- Phone storage options no longer mention "SAF folders".

### Phone screen on the PC: no more freezes when the phone turns

- Android (advanced mode): when the phone turns, for example when entering or leaving a
  landscape video, the screen mirror is made again from scratch instead of being resized in
  place. In place, Android 17 stopped sending pictures for 7–30 seconds.
- Linux: the Screen page's figures also show pictures replaced before being drawn, and the
  longest wait between packets and between pictures. `BROCHE_STREAM_LOG=1` writes them to the
  log every second.

### Phone screen on the PC: turning the phone's screen off works again, stopping from the phone

- Android (advanced mode): "Turn off the phone's screen" turns the panel off again on Android 14
  and newer, while the PC keeps showing it live. The panel's token now comes from
  `DisplayControl`, with its native library loaded as scrcpy does. Before, it failed silently
  and only the brightness went down. The panel goes off once frames arrive, and the
  brightness trick is only a fallback, since changing the brightness turns the panel back on.
- Linux: stopping the sharing from the phone no longer leaves the PC waiting for "Accept the
  request on the phone". This also covers "phone only" mode and other pages.

### Android: name this phone, number repeated PC names

- Settings › Devices starts with "Name of this phone" and a Change button. PCs see the new
  name at once. Empty goes back to the phone's own name.
- Linked PCs that share a name are listed as "fedora (2)", "fedora (3)"… in pairing order. The
  number goes away when one is renamed.

### Linux: name this PC

- Settings › Preferences › This PC and the first welcome page have "Name of this PC". The
  phone shows the new name at once. Empty goes back to the system's name.
- The default name is GNOME's "Device name" (`PRETTY_HOSTNAME` in `/etc/machine-info`) when
  set, else the host name.
- Linked devices that share a name are listed as "name (2)", "name (3)"…

### Core: device names that can change and never look the same twice

- `Node::set_device_name` (also in the FFI) renames this device; connected devices see it
  at once through the existing `hello.update`.
- `PairedDevice.display_name`: a name an earlier linked device already has gets " (2)", " (3)"…
  It is worked out when listed, so it goes away once either device is renamed.
- Device names, ours and received, are cleaned: no control characters, trimmed, at most
  40 characters.
- No visible change yet: Linux and Android use this in the next steps
  (`docs/superpowers/plans/2026-11-05-device-names.md`).

### Android: copying a password no longer shows "Shell pasted from your clipboard"

- In advanced mode the clipboard watcher looks at the clip's description first and skips
  passwords (and Broche's own clips) without reading them, so Android no longer shows its
  "pasted" notice. Passwords were already never sent.
- Fixed a guide test that still expected the old "Deck" name.

### Linux: notifications and window show Broche's name and icon

- `install.sh` writes the program's full path in the menu entry. GNOME Shell's `PATH` often
  lacks `~/.local/bin`, and then it ignored the entry: the window had a generic icon and
  notifications showed `io.github.robertcjla.Broche` instead of "Broche".

### Android and Linux: "PC shortcuts" instead of "Deck", guide texts for Linux

- The Deck is now called **PC shortcuts** (Atajos del PC) on both apps, so its name says what it
  does: buttons on the phone that do things on the PC. Its permission reads **Run shortcut
  commands**; on Linux the editor is **PC shortcuts on the phone** in Settings › Preferences.
- Android texts no longer talk about Windows or File Explorer: phone storage is browsed from
  Broche on the PC (Files and photos › Files), Wake-on-LAN points to the PC's network settings, and
  the storage guide names the per-PC **Browse this phone's storage** permission instead of the
  storage switch removed earlier.

### Linux: welcome and first setup

- The first time Broche opens it shows a welcome in five pages: language and light/dark mode;
  linking the phone with the QR code (or, if one is already linked, saying so); what the phone
  may do (permission profile); starting with the session and the automatic clipboard; and where
  things are. **Back**, **Next** and **Skip the welcome** on every page. It can be seen again from
  **Settings › About › Show the welcome again**.
- Existing installs see it once too (nothing is lost: it only shows the current choices).

### Linux: tips on Home

- Home shows a short tip about something Broche can do, picked at random each time Broche
  opens, with **Another tip** and **Hide** (until it opens again). **Settings › Preferences ›
  Window › Tips on Home** turns them off for good.

### Linux: automatic clipboard

- New switch **Settings › Preferences › Clipboard › Automatic clipboard** (off by default): what
  you copy on the PC goes to the phone on its own, and a notification says when the phone's
  clipboard arrives (the notification never shows what was copied). Copies marked secret by
  password managers are not sent, and what just came from the phone is not sent back. When off,
  use the **Send clipboard** buttons as before. Works because Broche runs through XWayland.
- Fixed: Broche's notifications (phone looking for this PC, phone files, clipboard) could vanish
  before GNOME showed them; they are now sent over one lasting D-Bus connection and say they
  are Broche's.

### Linux: always XWayland, simpler tray and settings

- Broche always opens through XWayland: the tray can bring the window forward and hide it,
  files can be dropped on it, and the clipboard can be watched later on. Plain Wayland is only
  the fallback when there is no X server. The **Compatibility mode (XWayland)** switch and the
  Wayland notes in Settings are gone. With fractional display scaling the window may look less
  sharp.
- **Use its camera** is no longer in the tray menu (the Camera page has it).
- The **Open it in window mode** setting is removed: from the tray the phone's screen always opens
  in window mode; inside Broche it shows on the Screen page, and **Show only the phone** still
  switches the window to window mode.

### Linux: screen readers skip decorative icons

- Buttons and options that start with an icon are read without it (*Ring*, not "bell Ring");
  decorative icons (phone, speaker, empty-page icons, the path separator in Files) are no longer
  read at all.
- Settings switches carry their explanation, so Orca reads what a switch does when it gets focus.

### Linux: tray folders and phone screen

- The tray's **Folders** submenu opens again: it closed the moment it opened, because Broche
  told GNOME the menu had changed every time it was shown.
- Folders opened from the tray come to the front (Broche now takes GNOME's activation token).
- **Show its screen** in the tray always opens the phone in window mode, bringing the window out
  of the dock; **Stop viewing its screen** minimizes it again instead of turning it back into the
  whole app. The tray's window-mode check item is gone: the preference in Settings is for the
  **Show its screen** button inside Broche.
- Settings explain what depends on the display mode: **Advanced › Compatibility mode (XWayland)**
  lists what it enables and which mode is in use now (and asks for a restart after a change), and
  **Phone's screen** warns on Wayland that the tray can't bring the window forward. The tray's
  "files are open in Broche" notification is only shown on Wayland, where it is needed.

### Linux: screen reader names and order

- Text fields, the device and app pickers, the photo album picker and the phone volume
  (slider and value box) now have names, so Orca says "Reply, entry" instead of just "entry".
- Keyboard and screen readers now reach a text field before its button (e.g. the clipboard text
  before **Send text**) and the phone's music controls in order: Previous, Play, Next.
- The search icon in Settings is no longer announced as an emoji.

### Linux: window mode for the phone's screen

- New preference **Settings › Preferences › Phone's screen › Open it in window mode**, also a
  check item in the tray menu (**Phone's screen in window mode**). When on, every phone screen
  you open starts as "Show only the phone"; switching it while the screen is showing applies at
  once.
- The tray's **Phone files** entry now also shows a notification saying the files are open in
  Broche, since Wayland does not let an app bring its own window to the front.

### Android: settings reorganized

- Settings › Devices shows, for each PC, the profile, the 18 switches folded under **Detailed
  permissions** (open when the profile is custom) and **Unpair** last. The separate "browse this
  phone's storage" switch is gone: it is part of the profile and the details, and turning storage
  on still asks for file access when the phone has none.
- The permissions category is now **Android permissions**, to tell it apart from what the PC
  may do; Elevated is offered as **Advanced mode (wireless debugging)**.

### Android: clearer errors from the PC

- When something is not allowed (by the PC's permissions for this phone or this phone's
  permissions for the PC), Android now says so and where to change it, instead of "Something
  went wrong". A file or folder that no longer exists on the PC says that too.
- Browsing the PC's files before Broche has started shows "not connected" instead of an empty
  folder.
- The remote-control and Deck "not allowed" hints point to the PC's current settings (Settings ›
  Device › Detailed permissions) and the permission's real name.

### Linux: settings reorganized

- Settings tabs are **Preferences, Device, Pair, About**. Device has the profile on top, the
  18 switches folded under **Detailed permissions** (open when the profile is custom) and
  Unpair last. The Activity tab moved into About as a folded **Technical log**. The XWayland
  compatibility mode moved to a folded **Advanced** card in Preferences.
- Spanish says **vincular** (like Android) instead of «emparejar».

### One video quality for both devices

- Video quality is one choice, **Normal (720p, 30 fps)** or **High (1080p, 60 fps,
  experimental)**, kept the same on the PC and the phone: change it on either. It replaces the
  separate 60 fps switches on each device and Android's webcam 1080p switch. New protocol message
  `settings.video`; the newer choice wins.

### Linux: phone only

- **Show only the phone** turns the window into the phone's screen, sized like the phone, with
  Back, Home and Recents and a button back to Broche (the size is given back). It is the same
  window on purpose: a second window froze while Broche was minimized (Wayland draws nothing for
  a minimized window, and egui draws its other windows from it).
- The Home key's icon in the Screen tab rendered as an empty box; it is a house now.

### Fixes from phone testing

- Linux: one Broche folder. Received files go to `~/Broche/Recibidos` (moved once from
  `~/Downloads/Broche`), next to what the phone may browse.
- Linux: the phone's **Task view** button (Screen tab and full screen) opens the overview: it
  sent a key Linux didn't know. Every mirror key is now checked by a test.
- Linux: Files' **Try again** is centred, and "phone not connected" no longer shows as a
  missing permission when the app is in Spanish.
- Android: the webcam sends what its preview shows (centre crop) instead of a narrow portrait
  picture between black bars when the phone is upright.
- Android: button labels in Control PC (Screen, Touchpad, Presentation) wrap to two lines
  instead of being cut, and buttons in a row share one height (`ButtonLabel`).

### Removed: phone brightness from the PC

- The PC can no longer change the phone's screen brightness: it often failed. Gone from
  the protocol (`toggles.brightness`, the `brightness` field and toggle ID), the Linux
  phone controls, the CLI, Android's "Screen brightness" setting and its
  `WRITE_SETTINGS` permission, and the Elevated helper's allowlist. Turning the phone's
  screen off while sharing it is unchanged.

### Linux app

- The settings search is a Material search bar (pill, magnifier, clear button) instead of
  a plain text field.

### Permission profiles

- Pick what a paired device may do as a profile instead of switch by switch: **Notifications
  only**, **Basic** (the defaults), **Multimedia** (Basic + browsing the phone's storage and
  seeing the PC's screen) or **Full control** (everything except files over relays). Changing
  a switch by hand shows the profile as custom.
- Linux: in Settings › Permissions, and asked right after pairing a phone (Basic preselected).
  Android: in Settings › Devices, after pairing and in the setup guide.
- Core: `features::PermissionProfile`, `profile_of`, `Node::apply_permission_profile` and
  `permission_profile` (also in the FFI).

### Linux app: missing programs

- About lists the features whose programs aren't installed (FFmpeg, PipeWire tools,
  GStreamer with an H.264 encoder, `gio`, `xdg-open`) and the one command that installs them
  on Fedora, Debian/Ubuntu, Arch or openSUSE, with a Copy button. Only the Fedora package
  names were checked on a real system.
- About no longer says new branding is planned: Broche has its own name and icon.

### Your own Deck buttons (Linux)

- Preferences › **Phone Deck buttons** adds buttons to a **Custom** page of the phone's Deck:
  open an app (from the menu), open a link (http/https) or run a command. Commands run
  without a shell, need the phone's "run commands" permission and a confirmation when saved.
  What a button does stays on the PC, in `~/.config/broche/deck.json`.
- Core: an app to launch is now a `.desktop` menu entry (it was a Windows `.exe`/`.lnk`), and
  commands must split cleanly into program and arguments.

### Linux app

- Error messages that come from the core ("device is offline", "network error: …") now
  read in Spanish when the app is in Spanish, in the status line and in Files.

### Renamed to Broche everywhere — update both apps

- **An updated device can't talk to one that isn't: update the Linux app and the phone
  together.** The pairing link is `broche://pair`, the QUIC protocols are `broche/0` and
  `broche-pair/0`, and the pairing proof labels changed. Device keys and stored pairings are
  kept, so an existing pairing should carry over once both are updated; pair again if not.
- Crates are `broche-*` (`core/broche-*`, `broche-gui`); the Android Kotlin package is
  `io.github.robertcjla.broche` and its native library `libbroche_ffi.so`; environment
  variables are `BROCHE_*` and the Gradle property is `broche.abis`.
- Android's notification access and the accessibility service must be granted again: their
  component names changed. Stored data keeps its names (`nectarlink.db`, Keystore alias), so
  settings and history survive the update.

### Unused code removed

- Core: the Windows build gate and the virtual-camera add-on requirement are gone, so
  **the phone-as-webcam feature now shows as available with a Linux PC** instead of asking
  for a Windows add-on. An unknown device kind running `linux` counts as a desktop.
- Removed core functions nothing called (queue clean-up, SMS send variants, storage read,
  timeline lookup, a Deck helper) and 11 FFI functions the Android app never used.
- Android: unused honeycomb constants, three helpers, the `primary` color and the
  `ic_music` icon. Removed NectarLink's brand images (`assets/brand`).

### Documentation

- New README for Broche; GitHub description and topics set. Removed NectarLink's Windows
  user guide, ADRs, research notes, old plans, performance and audit reports, the local HTTP
  API and the design mockups (all still in Git history). Technical docs rewritten for Linux
  and Android (building, releasing, architecture, storage, security); `docs/design/tokens.json`
  stays as the theme source.

### Removed: Windows

- Broche is Linux and Android only. Removed NectarLink's Windows app (Qt), virtual camera,
  installer, PowerToys extension, Qt spike, PowerShell scripts, `xtask` (theme tokens and
  Windows notices; `Tokens.kt` is now edited by hand), DPAPI and the Windows-only CLI code
  (watching a process by PID). The release workflow builds the Linux tar.gz instead of the
  Windows installer.

### Device key in the desktop keyring (Linux)

- The PC's device key, clipboard history and LocalSend certificate are now sealed with
  AES-256-GCM by a random key kept in the desktop keyring (Secret Service, or the Secret
  portal in a Flatpak), through `oo7`. The existing key is re-sealed on first start with the
  same identity, so phones stay paired. Without a keyring, the owner-only file is kept.
- **Needs a step from you only if you go back:** older Broche builds can't open a re-sealed
  key. Deleting the keyring item "Broche device key" means pairing again.

### Linux audio without extra packages

- "Phone as speaker" captures the PC's sound with PipeWire's `pw-record`, falling back
  to `parec`; "Find my PC" falls back to looping `pw-play` when `ffplay` is missing.
  Neither needs PulseAudio tools or FFmpeg on a PipeWire desktop any more.

### Dependency security updates

- Rust: tokio 1.54.1, rustls-webpki 0.103.16, webpki-roots/webpki-root-certs 1.0.11 and
  ctutils 0.4.8 (RUSTSEC-2026-0381/0382/0383/0385/0386).
- Android: require Bouncy Castle 1.85 (pulled in by libadb-android) and Guava
  33.7.2-android (pulled in by CameraX), fixing GHSA-574f-3g2m-x479, GHSA-9pwp-9qqc-pr26,
  GHSA-c3fc-8qff-9hwx, GHSA-qp49-qgx5-5m26 and GHSA-xxph-c9ww-hj94.

### Native file sends

- Close every file descriptor handed to a send call when it fails early: an invalid
  device ID, or one bad descriptor among several, no longer leaks the others.

### Android security and cleanup

- Retain the pending Android UI/video adjustments: audio controls in fullscreen Deck,
  nearby pairing selected initially, single-line Home button labels, and a 720p webcam
  default with an experimental 1080p preference in Settings.
- Add a permission-free cross-app probe, a packaged-handshake ART runner and a clear
  handoff documenting verified device checks and remaining validation.

- Stop automatically granting call-control and sensitive-notification app-ops
  from the Elevated helper; existing grants are not automatically revoked.
- Bound unauthenticated Elevated connections by concurrency, token size and a
  total deadline; remove leftover banking and radio shell commands.
- Isolate internal clipboard/file-picker actions from exported sharing and ask
  for confirmation before external shares; reject file URIs and own-provider URIs.
- Fix the Android 8–9 received-file provider path and close file descriptors
  when a send stops before native ownership begins.
- Remove unreachable Hive/Glass engines, unused Android translations and the
  Android-only packaging of the unused serif font; share the developer-settings
  launcher while preserving legacy theme migration and current palettes.

Everything Broche changed on top of NectarLink 0.1.4. The first Broche
release will be **0.2.0** (see docs/VERSIONING.md).

### New

- **Linux desktop app** (`desktop/linux-gui`): messages, calls, notifications,
  photos, phone storage, phone screen and camera on the PC, PC screen on the
  phone, remote input, phone as a speaker, PC media (MPRIS) on the phone,
  battery and resource monitor (CPU, memory, temperature) both ways, tray
  menu, start with the session, Flatpak/AppImage-aware autostart.
- Material 3 interface on Linux and Android with Catppuccin and Tokyo Night
  schemes, WCAG contrast checked by tests, adaptive navigation and text size.
- **English and Spanish** throughout: the Android app and the Linux app follow
  the system language; a test fails if a Linux text has no Spanish translation.
- Video capped at 30 fps by default, 60 fps as an experimental setting.
- Own name and icon: **Broche**, two interlocked hooks in blue and light blue
  (the working name Enlacea was dropped because others already use it); see
  docs/BRANDING_PLAN.md.

### Changed

- Linux: the phone's music has one play/pause button that shows what a click
  does (and the song playing); Settings has a search field that finds any
  setting by the words in it, without minding accents or case, and jumps to
  it; «Unpair…» is on the phone's card in Home.
- Android Home: the desk clock is gone and the PC's music controls (artwork,
  previous, play/pause, next, 48 dp targets) live in the PC card with the
  volume; Record, Lock PC and Sleep left Home and "Ring PC" sits next to
  Webcam; the PC monitor no longer shows the temperature; buttons have their
  own surface and a visible outline, section labels are full contrast, and
  labels wrap to two lines instead of being cut ("PC files", "Send folder").
- Android settings by category with search; secondary screens with a common
  top bar and written connection state; Deck status badges no longer cut off
  on narrow tiles, and the key name keeps its two lines below them.
- Fewer Android permissions: no Wi-Fi/Bluetooth/hotspot toggles; Elevated no
  longer grants sensitive notifications or call control.
- The device key file is created owner-only (0600) and its folder 0700;
  older files are tightened on the next start.
- **Phones browse `~/Broche`, not your whole Home folder.** Builds that shared
  Home switch to it on start; a folder you chose yourself is kept, and each
  phone keeps the storage permission it had.
- Linux app ID `io.github.robertcjla.Broche`, commands `broche` and
  `broche-cli`; settings, pairings and received files move to
  `~/.config/broche`, `~/.local/share/broche` and `Downloads/Broche` on the
  first start (renamed in place, nothing copied or deleted).
- Android app ID `io.github.robertcjla.broche`: it installs as a new app, so
  **the phone must be paired again**; received files go to `Download/Broche`.
- CI builds and tests the core, the Linux app and the Android app only; the
  release workflow runs only by hand. It also checks formatting
  (`cargo fmt`) and fails on any clippy warning, Linux app included.

### Removed

- Auto-updater and Pause for banking apps (with their permissions).
- Simplified Chinese on Android (it was incomplete for Broche's new texts).
- Windows-only CI jobs.

### Fixed

- Android setup no longer fades the top of step content or repeats the progress header; the obsolete honeycomb illustration is removed.
- PC files open as a full-screen Android route using the activity system bars; listing errors are shown instead of appearing as an empty folder.
- History and clipboard cards use centered content, consistent padding and wrapping outlined actions; Timeline is labeled History in English and Spanish.

- Phone screen and camera on the PC froze after a few frames (decoder now
  outputs PPM and restarts itself); PC screen frames were being dropped.
- Speaker switch turned itself off when the volume changed.
- LocalSend crash when toggled; remote file lists hid permission errors.
- Tray deadlock that froze the window; stale Files error after reconnecting.
- **LocalSend uses encryption only:** plain HTTP is neither served nor used,
  peers with encryption off are ignored, and a peer's certificate must match the
  fingerprint it announced (before, any certificate was accepted, so another
  machine on the network could answer in its place).
- **The phone's sound never played on the PC** while its screen was shared:
  FFmpeg 7+ refuses `-ac` on raw input, so `ffplay` quit at once. It now plays
  through PipeWire's `pw-play` (any modern desktop), with a fixed `ffplay` as
  fallback. (With Elevated mode Android moves the sound to the PC, so the
  phone itself goes quiet while it is shared.)
- Sharing the PC's screen needed GStreamer's `openh264enc`, which Debian and
  Ubuntu don't ship; it now uses the first of `openh264enc`, `x264enc`,
  `vah264enc` and `vaapih264enc` it finds, and says so if there is none.
- With the webcam running, "Take photo" and "Scan document" froze the stream
  and its torch: a photo is now the frame already showing (saved as PNG in
  Downloads/Broche), and a scan stops the webcam first.
- GNOME's screen-sharing indicator stayed on after the phone left the remote
  screen: the desktop session now closes after two idle minutes.
- The desk clock did not follow a language change (Android lint error).
- Pausing or resuming a transfer that had not started yet (queued, or waiting
  for the device) was ignored, so it was sent anyway on reconnect.
