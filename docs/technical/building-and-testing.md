# Building and Testing

## Toolchains

| Tool | Version | Notes |
|---|---|---|
| Rust | stable channel in `rust-toolchain.toml`; `Cargo.toml` needs 1.97+ (edition 2024) | `rustup` installs it; Android targets `aarch64-linux-android`, `x86_64-linux-android` |
| C compiler | `gcc` | bundled SQLite, ring |
| Linux runtime tools | PipeWire (`pw-play`, `pw-record`, `wpctl`), `ffmpeg` (decodes phone video), GStreamer `openh264` (PC screen to phone) | optional fallbacks: `parec`, `ffplay` |
| Android | JDK 17, SDK platform 37, NDK, `cargo-ndk` | `ANDROID_HOME`, `JAVA_HOME` |

## Linux app and CLI

```sh
cargo build --release -p broche-gui -p broche-cli   # target/release/broche, broche-cli
APPIMAGE=0 scripts/package-linux.sh                          # tar.gz in target/package/
target/package/Broche-*-linux-x86_64/install.sh              # installs into ~/.local
```

## Android

```sh
cd apps/android
./gradlew assembleDebug -Pbroche.abis=arm64-v8a   # one ABI, faster
./gradlew assembleRelease                         # default ABIs (arm64-v8a, x86_64), R8
```

Gradle builds the core with `cargo-ndk` and generates the Kotlin bindings.

## Tests (what CI runs)

```sh
cargo fmt --all --check
cargo clippy --locked -p broche-protocol -p broche-core -p broche-ffi -p broche-cli -p broche-gui --all-targets -- -D warnings
cargo test --locked -p broche-protocol -p broche-core -p broche-ffi -p broche-cli -p broche-gui --lib --bins
cargo test --locked -p broche-core --tests     # two real nodes over the network
(cd apps/android && ./gradlew testDebugUnitTest lintDebug assembleDebug)
```

The two-device tests need a network interface with an address. Scale tests are
opt-in (`cargo test -p broche-core --test scale_tests -- --ignored`).

## Debugging the Linux app

Environment variables read by `broche`:

| Variable | Effect |
|---|---|
| `BROCHE_DATA_DIR=<dir>` | Another identity and data folder, to test without touching the real one. `broche` and `broche-cli` share the default one: do not run both at once. |
| `BROCHE_CONFIG_DIR=<dir>` | Another preferences folder (theme, language). |
| `BROCHE_LOG=<filter>` | Log level of the core, as a `tracing` filter (default `warn,zbus=error`). |
| `BROCHE_DUMP_DIR=<dir>` | Saves each received phone screen/camera stream as `<dir>/*.h264`. |
| `BROCHE_STREAM_LOG=1` | Logs the phone screen/camera counters every second (packets, frames, errors, dropped, longest network and picture gap). |
| `BROCHE_SHOT=<dir>` | Visits the pages, saves a screenshot of each and quits. `BROCHE_SHOT_SIZE=640x720` sets the window size, `BROCHE_SHOT_PAGES=…` the pages, `BROCHE_SHOT_WAIT=<s>` the delay; `BROCHE_TEST_STORAGE=1` shows sample folders in Files. |
| `BROCHE_WELCOME=<page>` | Opens the welcome at that page (1–5), even if it was already seen. |
| `BROCHE_AUTOSTART=screen\|camera\|shelf\|app:<package>` | Starts the phone's screen or camera, opens the shelf, or opens that phone app in its own window, on connect. |
| `BROCHE_TEST_SCREEN=1` | Shares a test picture instead of the real PC screen. |

A simulated phone, with its own data, for screenshots and tests (run it without
`--no-lan` for the PC to find it):

```sh
broche-cli --data-dir /tmp/demoA --no-lan pair            # prints a broche://pair?... link
broche-cli --data-dir /tmp/demoB --as-phone --no-lan join "<link>"
broche-cli --data-dir /tmp/demoB --as-phone demo --name "Test phone"
BROCHE_DATA_DIR=/tmp/demoA broche
```
