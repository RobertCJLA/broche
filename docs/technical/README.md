# Broche technical documentation

Engineering notes for Broche, an independent build of NectarLink 0.1.4 for
**Linux and Android**: the shared Rust core, the Linux desktop app (egui) and
CLI, the Android app (Jetpack Compose + UniFFI), the wire protocol and storage.

| Document | Scope |
|---|---|
| [Architecture](architecture.md) | Crates, runtime topology, discovery and transport, the Android advanced-mode helper, Bluetooth calls |
| [Security Model](security-model.md) | Device identity and key protection, pairing, `CapabilityMatrix`, consent, path checks |
| [Protocol Overview](protocol-overview.md) | Wire framing, streams, handshake, transfers, media; index of [`docs/protocol/`](../protocol/v0.md) |
| [Storage and Retention](storage-and-retention.md) | SQLite schema, send queue, encrypted clipboard history |
| [Building and Testing](building-and-testing.md) | Toolchains, build, test and package commands |
| [Releasing](releasing.md) | Versions, tags and the manual release workflow |
| [Contributing](contributing.md) | Licenses, headers, code style, protocol and migration rules |

## Repository map

```text
├── Cargo.toml               Workspace manifest and shared dependency versions
├── apps/
│   ├── linux/               Linux app `broche` (egui, tray, portals, PipeWire) and packaging
│   ├── android/             Android app
│   └── cli/                 `broche-cli`
├── crates/
│   ├── broche-protocol/     Wire framing, CBOR envelopes, capability names, SAS
│   ├── broche-core/         QUIC node, sessions, pairing, SQLite store, send queue, keyring
│   ├── broche-adb/          ADB host protocol for the USB tunnel
│   └── broche-ffi/          UniFFI bindings for Android
├── scripts/                 `package-linux.sh`, `release-local.sh`, `bump-version.sh`,
│                            `check-public.sh`, `flatpak-cargo-sources.py`
└── docs/                    protocol/, technical/, architecture/, branding/, design/
```
