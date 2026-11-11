# Broche for Android

Kotlin + Jetpack Compose (Material 3) on top of the shared Rust core,
called through UniFFI (`crates/broche-ffi`). Android 8 (API 26) or later;
targets API 37.

## Build

Needs JDK 17, the Android SDK (platform 37, NDK 30.0.16248370), Rust with the
Android targets, and `cargo-ndk`:

```
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
```

Then, from `apps/android/`:

```
./gradlew assembleDebug                          # phones + x86_64 emulator
./gradlew assembleDebug -Pbroche.abis=arm64-v8a  # phones only (faster)
./gradlew assembleDebug -Pbroche.abis=x86_64     # emulator only
./gradlew testDebugUnitTest
```

Gradle builds the core with cargo-ndk (cargo profile `android`: release
optimizations, symbols kept for UniFFI and stripped when packaging) and
generates the Kotlin bindings from the built library. Both are generated
sources; nothing generated is checked in. The theme tokens in `ui/theme/Tokens.kt`
are edited by hand.

## Layout

Code lives in `app/src/main/kotlin/io/github/robertcjla/broche/`; texts in
`res/values/` (English) and `res/values-es/` (Spanish).

- `core/`: the bridge to the Rust core. `Core` owns the node and folds its
  events into `CoreState` (a pure reducer, unit-tested); `KeystoreKeyProtector`
  encrypts the identity key with a non-exportable Android Keystore key;
  `Ringer` rings on the alarm stream; battery, signal and network monitors feed the
  core (Android doesn't let native code watch the network).
- `service/ConnectionService`: a "connected device" foreground service that
  keeps the phone reachable and holds the Wi-Fi multicast lock local
  discovery needs; `StartupReceiver` starts it after a restart.
- `notifications/`: the notification listener (with the user's notification
  access) that hands notifications to the core for mirroring, and reads them
  (messaging-style content, actions, app icons); replies and actions from a
  PC run through the core's platform callbacks.
- `calls/`: call state for PCs (`PhoneCalls`), the in-call service that lets a
  PC answer, mute or hold (`CallCompanion`), and caller names.
- `sms/`, `contacts/`, `photos/`, `storage/`, `toggles/`: what a PC may read or
  change, each behind its own permission and per-device toggle.
- `clipboard/`: `SendActivity` receives external shares and asks for confirmation
  in a themed window (`ShareSheet.kt`: preview, target PC, Send or Copy). Its alias
  `CopyToPcClipboard` is a second share entry that puts an image on the PC's clipboard.
  External file shares accept content providers other than Broche's own provider;
  file URIs are rejected. `InternalSendActivity` is not exported and handles the
  clipboard tile, notification button and internal file picker. Clipboard reads
  happen only when that internal activity has focus.
- `files/`: opening files to send, publishing received files to
  Download/Broche (MediaStore on Android 10+), and the transfer
  notifications with progress and Cancel.
- `media/`: this phone's media sessions for PCs (`PhoneMedia`, with
  notification access), and a PC's player in Android's media controls
  (`PcMedia`, a media session and its notification).
- `mirror/`: the phone's screen for PCs (`MirrorService`, `ScreenEncoder`, its
  sound in `SoundCapture`), touch from the PC (`InputService`, an accessibility
  service) and phone apps in windows of their own (`AppWindows`).
- `webcam/`, `camera/`: the phone as the PC's camera, and taking a photo or
  scanning a document for the PC.
- `audio/`: the phone as the PC's speaker (`PcSpeakerPlayer`).
- `elevated/`: **advanced mode**, below.
- `widget/`, `links/`: the home-screen widget, and links sent from the PC.
- `ui/`: pairing (QR scanner with CameraX + ZXing, no Google Play services;
  nearby pairing with a 6-digit code), the walkthrough and features guide,
  Home, Activity and Settings, PC shortcuts (`deck/`) and the remote. Catppuccin
  and Tokyo Night palettes are available; the system palette uses Android 12+
  dynamic colors.

The identity lives in `noBackupFilesDir`, so it's never restored onto
another phone. `broche://pair?...` links (a PC's QR code scanned with any
camera app) open a confirmation dialog before pairing.

## Release and signing

Release builds enable R8 code and resource shrinking (`isMinifyEnabled = true`,
`isShrinkResources = true` with `proguard-rules.pro` keeping JNA, UniFFI
bindings, Conscrypt, and the advanced-mode helper entry point).

No signing keys or passwords are stored in the repository:

- Set `BROCHE_KEYSTORE` (path to the `.jks` keystore, key alias `broche`) and
  `BROCHE_KEYSTORE_PASSWORD` in the environment before running
  `./gradlew assembleRelease`. `scripts/release-local.sh` does this and checks
  the signature (see [docs/technical/releasing.md](../../docs/technical/releasing.md)).
- In GitHub Actions (`.github/workflows/release.yml`, run by hand), these come
  from the `ANDROID_KEYSTORE` (base64) and `ANDROID_KEYSTORE_PASSWORD` secrets,
  and the temporary `.jks` file is deleted right after `apksigner verify`.
- When `BROCHE_KEYSTORE` is unset, `assembleRelease` signs `app-release.apk`
  with the debug key, for testing only; never publish that build.

Broche has no in-app updater.

## Advanced mode and permissions

Advanced mode (`elevated/`) is optional. It uses the phone's wireless debugging
after user setup (or root, on rooted phones) to run a helper with the shell's
permissions: real touch and keys on the mirrored screen, and phone apps in
windows of their own. It never grants call control (`MANAGE_ONGOING_CALLS`) or
sensitive notifications (`RECEIVE_SENSITIVE_NOTIFICATIONS`) by itself: while it
runs, Settings shows a switch for each, off by default, and only those switches
ask the helper to turn them on or back to Android's default (`Elevated.setAppOp`,
checked by `ElevatedGrantsTest`). Permissions granted by an earlier
build are not automatically revoked.

Its loopback helper authenticates with a random token: at most eight handshakes
run concurrently, each limited to 64 input bytes and a five-second total deadline.
Authenticated control, audio and video streams are not subject to that deadline.
The helper's shell-command allowlist only keeps notification expansion; brightness,
banking and radio controls were removed and are not accepted.

Received files use MediaStore on Android 10+. Android 8–9 uses the app-specific
`Download/Broche` directory through FileProvider.

## Checks

```sh
./gradlew testDebugUnitTest lintDebug assembleDebug -Pbroche.abis=arm64-v8a
./gradlew assembleRelease -Pbroche.abis=arm64-v8a
```

JVM tests cover the state reducer, share URI policy, caller names, document
scanning, MMS parsing, storage paths, helper authentication
deadlines and limits, removed privileged grants, descriptor ownership and the
FileProvider path declaration. They do not replace tests of exported
activities, Android 8–9 FileProvider behavior, or advanced-mode audio/video on a
real device.
