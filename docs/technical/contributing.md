# Contributing

This guide summarizes the repository conventions, module header requirements, protocol change rules, and pre-submission checks for contributing to Broche. See also the root [`CONTRIBUTING.md`](../../CONTRIBUTING.md) and [`LICENSE`](../../LICENSE).

## License

Broche keeps NectarLink's three licenses across the repository (see [`LICENSE`](../../LICENSE)):
- **Core libraries** (`crates/broche-protocol`, `crates/broche-core`, `crates/broche-ffi`): [`MPL-2.0`](../../LICENSES/MPL-2.0.txt)
- **Applications and CLI** (`apps/linux`, `apps/android`, `apps/cli`): [`GPL-3.0-or-later`](../../LICENSES/GPL-3.0-or-later.txt) with an [`app-store exception`](../../LICENSES/APP-STORE-EXCEPTION.md)
- **Protocol specification and documentation** (`docs/`): [`CC BY 4.0`](../../LICENSES/CC-BY-4.0.txt)

Contributions are accepted under the [`Developer Certificate of Origin`](../../DCO.txt).

## SPDX Header Policy

Every Rust (`.rs`) and Kotlin (`.kt`) source file must start with the appropriate `SPDX-License-Identifier` comment:

```rust
// SPDX-License-Identifier: MPL-2.0
```

or for application and CLI files:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
```

- Never log message text, phone numbers, contact names, file paths, or clipboard contents (see [`docs/protocol/v0.md`](../protocol/v0.md) section 11).

## Code Style and Static Analysis

### Rust

- **Formatting**: Format all Rust code with `cargo fmt --all`. Configuration lives in [`rustfmt.toml`](../../rustfmt.toml).
- **Lints**: Workspace-wide lints are configured in the root [`Cargo.toml`](../../Cargo.toml) (`unsafe_code = "deny"`, `missing_debug_implementations = "warn"`, every Clippy `all` lint as a warning). All code must pass the CI's Clippy command (below, with `--all-targets`) with zero warnings.
- **Error Handling**:
  - Use typed `thiserror` enums across the public APIs (`broche_core::Error`, `broche_protocol::ProtocolError`, and per-feature ones such as `StorageError`).
  - Never unwrap fallible network, CBOR, file I/O, or SQLite operations on untrusted peer inputs.
- **`unsafe` Blocks**:
  - `unsafe` is denied workspace-wide.
  - Where FFI calls require it (such as adopting Android file descriptors in `broche-ffi`), allow it for that block only, keep it minimal and document the ownership invariants.

### Kotlin (Android)

- Follow standard Kotlin conventions (`kotlin.code.style=official` in `apps/android/gradle.properties`).
- Run blocking disk, SQLite, or FFI calls on `Dispatchers.IO`, never on the main UI thread.
- Keep `Compose` list items free of per-frame allocations (such as `SimpleDateFormat` creation inside `LazyColumn` item blocks) to preserve the `< 5%` scroll jank budget.

## Adding or Modifying a Capability

When introducing a new cross-device capability or extending an existing wire message:

1. **Specification First (`docs/protocol/`)**: Document the capability ID, stream framing, CBOR message schemas, and error handling in [`docs/protocol/`](../protocol/v0.md) and register the capability in [`docs/protocol/capabilities.md`](../protocol/capabilities.md).
2. **Protocol Constants & Types (`crates/broche-protocol`)**:
   - Add the message types and constants in `src/messages.rs` (each service has its own module there, such as `messages::storage`).
   - Define the serde-serializable structs in `src/messages.rs` using `#[serde(default)]` / `#[serde(skip_serializing_if = "Option::is_none")]` so wire changes remain backward-compatible.
3. **Authorization Mapping (`crates/broche-core/src/features.rs`)**:
   - Add the feature to `FEATURES` with the capabilities it needs, and guard it with an existing or new toggle in `DEVICE_TOGGLES`.
   - Decide whether the feature is default-on or high-sensitivity opt-in (`default_enabled: false`), and whether it must be restricted over `ConnectionPath::Relay` (**Connect over mobile data** / `away_mode`).
4. **Core & FFI Plumbing (`crates/broche-core`, `crates/broche-ffi`)**:
   - Implement the handler in `broche-core` and check the peer's toggle (`Shared::toggle_on`) on both inbound and outbound paths, as the existing services do (for example `sms.rs`).
   - If Android needs the API, expose it in `crates/broche-ffi/src/lib.rs`; Gradle regenerates the Kotlin bindings on the next build.
5. **Two-Device Integration Test (`crates/broche-core/tests/two_devices.rs`)**:
   - Add an end-to-end test spinning up two in-process `Node` instances over QUIC to verify capability negotiation, happy-path execution, and toggle denial behavior.

## Database Migrations (`crates/broche-core/src/store.rs`)

If a change alters the SQLite schema:

1. Append one step to `MIGRATIONS` in `crates/broche-core/src/store.rs` (never edit a released step); `migrate` applies the missing steps and sets `PRAGMA user_version` to their count.
2. Use `ALTER TABLE … ADD COLUMN` or `CREATE TABLE IF NOT EXISTS` so existing user databases upgrade without data loss.
3. Add a unit test in `store.rs` covering a fresh database and the migration from the previous version.

## Pre-Submission Checklist

Before opening a pull request, run what CI runs (also in
[building-and-testing.md](building-and-testing.md)):

```sh
cargo fmt --all --check
cargo clippy --locked -p broche-protocol -p broche-core -p broche-ffi -p broche-cli -p broche-gui --all-targets -- -D warnings
cargo test --locked -p broche-protocol -p broche-core -p broche-ffi -p broche-cli -p broche-gui --lib --bins
cargo test --locked -p broche-core --tests
scripts/check-public.sh --all
(cd apps/android && ./gradlew testDebugUnitTest lintDebug assembleDebug)
```

Every new user-facing text needs its Spanish translation (`apps/linux/src/i18n.rs`, checked by
the `every_text_has_a_spanish_translation` test; Android `res/values-es/`).
