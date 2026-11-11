# Security Model

Broche uses an end-to-end encrypted, mutually authenticated peer-to-peer security model with no central user accounts and no cloud storage. Every connection is authenticated at the transport layer by static Ed25519 device identities and authorized at the application layer by per-device capability toggles and interactive runtime consent gates.

NectarLink's original audit record was removed with the Windows parts; it is in Git history (`docs/SECURITY-REVIEW.md`).

## Threat Model

| Adversary / Environment | Assumed Capabilities | Mitigations |
|---|---|---|
| Untrusted LAN / Public Wi-Fi | Can eavesdrop, replay, inject, or MITM UDP/TCP packets; can broadcast spoofed mDNS records | QUIC + TLS 1.3 (`iroh`) with mutual Ed25519 certificate verification; pairing requires either an out-of-band 128-bit QR secret (`HMAC-SHA256`) or a commitment-bound 6-digit SAS confirmed on both screens |
| Compromised Relay Server (**Connect over mobile data** / `away_mode`) | Can observe outer UDP packet sizes/timing and drop traffic | End-to-end TLS 1.3 termination between peer `DeviceId` keys; relay never possesses session keys; high-risk capabilities (`storage` without `remote_files`) are blocked on `ConnectionPath::Relay` |
| Unprivileged Local App on Android | Can attempt to connect to local ports or read app storage | Advanced-mode helper (`InputServer`) binds `127.0.0.1:<ephemeral_port>` and requires a 192-bit (24-byte / 48-hex-char) per-session `SecureRandom` token on the first line, verified in constant time (`MessageDigest.isEqual`) before accepting any commands |
| Offline Disk Inspection | Read access to `~/.local/share/broche` or the Android app data directory | Private identity key (`identity.key`) and clipboard vault (`clipboard_history.enc`, `clipboard_images/*.enc`) are sealed with the Linux desktop keyring (AES-256-GCM with a wrapping key in the Secret Service; owner-only files without a keyring) or Android Keystore AES-256-GCM (hardware-backed when supported by the device) |
| Paired Device Exceeding Intended Access | Attempts to invoke disabled capabilities, traverse outside shared paths, or start silent screen/input capture | Every feature handler checks the peer's toggle (`Shared::toggle_on`) before doing anything, paths are validated and resolved inside the shared root, and sensitive sessions (`pc_mirror`, `remote_input`, `webcam`) need the user's confirmation |

## Cryptographic Device Identity

Implemented in `crates/broche-core/src/identity.rs`.

- Each installation generates a 32-byte Ed25519 secret key (`iroh::SecretKey`) using the OS CSPRNG (`rand::rng()`).
- The corresponding 32-byte Ed25519 public key (`iroh::EndpointId`) serves as the canonical `DeviceId` (rendered in logs and URIs as z-base-32, with a 16-character `short()` prefix for UI display).
- On disk, `identity.key` is stored with a 5-byte header:
  - Bytes `0..4`: Signature `NLK1` (`b"NLK1"`).
  - Byte `4`: `KeyProtector` ID (`0` = plain, owner-only file, for headless CLI/tests, `1` = Windows DPAPI (NectarLink only, removed), `2` = Android Keystore, `3` = Linux desktop keyring).
  - Bytes `5..`: Sealed ciphertext.
- **Android Keystore Protection (`KeystoreKeyProtector.kt`)**: Generates a non-exportable 256-bit AES-GCM key (`alias = "broche.identity.v1"`) inside `AndroidKeyStore` (`PURPOSE_ENCRYPT | PURPOSE_DECRYPT`, `BLOCK_MODE_GCM`, `ENCRYPTION_PADDING_NONE`, which may be hardware-backed on devices with StrongBox or TEE) and seals secrets as `12-byte IV || ciphertext || 128-bit auth tag`.
- **Linux Desktop Keyring (`keyring.rs`, feature `secret-service`, used by Broche's app and CLI)**: a random 32-byte wrapping key is stored through `oo7` in the Secret Service (or the Secret portal's encrypted file inside a Flatpak) under attributes `application=io.github.robertcjla.Broche`, `purpose=device-key-wrap`. Data is sealed as `NLS1 || 12-byte nonce || AES-256-GCM ciphertext || tag` (AAD `NLS1`). A key file with method `0` is re-sealed on first load with the same identity; unsealed clipboard/LocalSend files are read as they are and sealed on the next save. If the keyring is unavailable at startup, a new installation falls back to method `0` (owner-only file); a method `3` file then refuses to load with an "unlock the keyring" error instead of creating a new identity.
- On Linux and Android, file permissions are restricted to the owner immediately upon creation. Legacy raw 32-byte key files are transparently migrated into `NLK1` sealed format on first load.

## Pairing Protocols (`ALPN_PAIR = "broche-pair/0"`)

Implemented in `crates/broche-protocol/src/pairing.rs` and `crates/broche-core/src/pairing.rs`. Pairing runs over a dedicated QUIC ALPN (`broche-pair/0`) and enforces a 5-minute expiration (`PAIRING_TTL = 300s`) and a maximum of 5 failed attempts (`MAX_FAILED_ATTEMPTS = 5`) before invalidating the pairing session.

### 1. QR Code Pairing Flow

```text
Host (displays QR)                                      Joiner (scans QR)
------------------                                      -----------------
Generates 128-bit secret S (16 bytes)
Encodes broche://pair?v=0&id=<host_id>&s=<base64url_S>&n=<name>&a=<ip:port>

                  ◄─── QUIC + TLS 1.3 (ALPN: broche-pair/0) ───
                  ◄─── pair.request { id, name, kind, proof } ─────
                       where proof = HMAC-SHA256(S,
                         "broche-pair-v0" || joiner_id || host_id)

Verifies TLS peer == joiner_id
Verifies HMAC proof in constant time
                  ──── pair.accept { id, name, kind } ────────────►
                  ◄─── pair.done ──────────────────────────────────
Persists peer to SQLite                                 Persists peer to SQLite
Closes with CLOSE_PAIRED (0)
```

- Because the 128-bit secret `S` is transferred optically via the QR code (`s` encoded in URL-safe base64 without padding) and the HMAC binds both `joiner_id` and `host_id` verified against the TLS 1.3 peer certificates, an active network attacker cannot forge `pair.request` or substitute their own `DeviceId`.

### 2. Nearby Discovery and 6-Digit Code Flow

When pairing via mDNS discovery or manual IP without scanning a QR code, both devices execute a commitment-based Short Authentication String (SAS) ceremony:

1. **Initiation**: Joiner sends `pair.request` with `proof: None`.
2. **Commitment Exchange**:
   - Joiner (initiator `A`) generates a 128-bit random nonce `nA` and sends `pair.commit { commit: SHA-256(nA) }`.
   - Host (responder `B`) generates a 128-bit random nonce `nB` and replies with `pair.nonce { nonce: nB }`.
   - Joiner reveals `pair.reveal { nonce: nA }`.
   - Host verifies `SHA-256(nA) == commit` without early exit (`verify_commitment`).
3. **SAS Calculation**: Both sides compute a 6-digit numeric code:
   - `SAS = u32_be(HMAC-SHA256(nA || nB, "broche-sas-v0" || idA || idB)[0..4]) mod 1,000,000` (`sas_code`; `idA` is the initiator's 32-byte Ed25519 public key and `idB` the responder's).
4. **Two-Sided User Confirmation**: Both devices display the 6-digit code and require explicit user approval (`pair.confirm { accept: true }`). Neither side writes the peer to SQLite until `pair.done` completes the two-phase commit.

## Session Authentication (`ALPN_SESSION = "broche/0"`)

Every operational session connects over `ALPN_SESSION` (`"broche/0"`):

1. During the QUIC + TLS 1.3 handshake, `iroh` cryptographically verifies that the remote peer holds the private key for its claimed `EndpointId` (`DeviceId`).
2. Immediately upon connection acceptance (`node.rs`), the node looks up the remote `DeviceId` in the SQLite `peers` table (`store.is_paired(&peer_id)`).
3. If the `DeviceId` is not present in `peers`, the connection is closed with the `UNPAIRED` application close code (`2`) before any control stream or capability handler is invoked.

## Authorization and the Capability Matrix

Implemented in `crates/broche-core/src/features.rs`. Every paired device has an independent set of 17 toggles (`DEVICE_TOGGLES`); changes from the defaults are stored in the `peer_toggles` SQLite table. Permission profiles (`PermissionProfile`) set them all at once when pairing.

### Default-On vs Opt-In Toggles

| Category | Toggle Keys | Default State |
|---|---|---|
| Standard Continuity (12) | `notifications`, `messages`, `calls`, `contacts`, `clipboard`, `files`, `media`, `photos`, `toggles`, `pc_actions`, `mirroring`, `webcam` | Enabled (`true`) upon pairing |
| High-Sensitivity / Opt-In (5) | `storage`, `pc_mirror`, `remote_input`, `commands`, `remote_files` | Disabled (`false`) upon pairing; must be explicitly enabled by the user |

### Inbound Enforcement

Each service checks its toggle for the peer (`Shared::toggle_on`, as in `sms.rs`, `photos.rs`, `contacts.rs`) before answering a request, and before sending anything to that peer; a refused request gets a `DENIED` error. The capability matrix (`features::compute`) only decides what the UI offers and explains what would unlock it.

### Relay Restrictions (**Connect over mobile data** / `away_mode`)

When a session is connected over `ConnectionPath::Relay` (**Connect over mobile data** / `away_mode`), `Node` applies additional transport-aware authorization:

- Remote file system browsing and downloads (`storage.read`, `storage.write`) are blocked on relay connections unless the user has enabled **both** the `storage` toggle and the opt-in `remote_files` toggle for that peer.
- Relay discovery and relay servers remain completely disabled (`RelayMode::Disabled`) unless the user explicitly turns on **Connect over mobile data** (`away_mode`).

## Runtime Consent Gates

Even when a capability toggle is enabled, sensitive operations require interactive user confirmation:

1. **PC Screen on the Phone (`mirror.capture`)**: When a phone asks to view the PC screen, the core emits `NodeEvent::PcMirrorRequested` and streams nothing until the user approves (`set_pc_mirror_session_allowed`); the desktop portal also asks which screen to share. While it runs, the Linux app shows a **Stop** button.
2. **Remote Input Control (`input.inject`)**: Synthesizing mouse and keyboard input on the PC requires both the opt-in `remote_input` toggle and interactive session approval (`NodeEvent::RemoteInputRequested`), plus the desktop's RemoteDesktop portal.
3. **Phone Storage Browser (`storage.read`, `storage.write`)**: needs the opt-in `storage` toggle and Android's `MANAGE_EXTERNAL_STORAGE` permission; deleting needs an explicit confirmation from the PC (`PhoneStorage.delete(confirmed)`).
4. **Phone-as-Webcam (`camera.stream`)**: Starting a camera stream from the PC notifies the phone and allows the phone user to stop or flip the camera at any time.
5. **LocalSend Incoming Files**: Unauthenticated LAN LocalSend peers never bypass pairing; every incoming LocalSend offer enters `TransferState::Requested` and waits for explicit user acceptance before writing files to the Downloads directory.

## File System and Path Traversal Protections

- **Incoming Transfers (`crates/broche-core/src/transfer.rs`)**: file names must be plain names (`is_valid_file_name`: no `/`, `\\`, control characters, `.` or `..`) and folders 1–32 such names (`is_valid_folder`); `safe_file_name` and `storable_name` clean what a sender gives. Writes stay in `<data_dir>/incoming/<peer>/<transfer_id>/` until complete, then move into the downloads folder.
- **Storage Browser**: request paths must pass `is_valid_storage_path` (relative, no `.`/`..` segments). On the phone, `storage/PhoneStorage.kt` resolves them with `canonicalFile` and refuses anything outside the canonical storage root, so symlinks can't escape it; on Linux, the shared folder (`FolderStorage`) blocks private and sensitive folders the same way.
