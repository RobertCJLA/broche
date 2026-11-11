# Storage and Retention

Broche persists all relational state in an embedded SQLite database (`broche.db`) managed by `crates/broche-core/src/store.rs`, supplemented by a `KeyProtector`-encrypted clipboard vault (`crates/broche-core/src/clipboard_history.rs`).

## Data Directory Layout

- **Linux (`~/.local/share/broche`)**, shared by `broche` and `broche-cli`, or `--data-dir` / `BROCHE_DATA_DIR`; the Flatpak uses `~/.var/app/io.github.robertcjla.Broche/data/broche`
- **Android (`noBackupFilesDir/broche`)**, never backed up or restored onto another phone

```text
<data_dir>/
├── identity.key                  Ed25519 secret key (NLK1 header + keyring/Keystore ciphertext)
├── broche.db                     SQLite database (WAL mode, 9 migrations)
├── clipboard_history.enc         KeyProtector-encrypted clipboard index
├── clipboard_images/
│   └── <clip_id>.enc             KeyProtector-encrypted images
├── incoming/
│   └── <peer_id>/<transfer_id>/  In-flight .part files for resumable transfers
├── queue_staging/
│   └── <queue_id>/               Staged copies of ephemeral Android ContentResolver FDs
├── localsend.enc                 KeyProtector-encrypted LocalSend certificate and settings
├── port                          The QUIC port this device listens on
└── instance.sock                 Single-instance socket of the Linux app
```

## SQLite Schema (9 migrations)

`crates/broche-core/src/store.rs` opens the database with `PRAGMA journal_mode = WAL` and `PRAGMA foreign_keys = ON`. `PRAGMA user_version` counts the applied steps of `MIGRATIONS` (9 today); a database from a newer Broche is refused instead of being changed.

| Table | Primary key | Purpose | Retention |
|---|---|---|---|
| `peers` | `id` (32-byte `BLOB`) | Paired devices: `name`, `kind`, `os`, `os_ver`, `model`, `accent`, `paired_at`, `last_seen`, `last_addrs`, last known `caps` and `power`, `wake_info` (Wake-on-LAN), `screen`, `install_id` | Removed when unpaired |
| `peer_toggles` | `(peer, toggle)` | Per-device toggle overrides (`enabled` 0/1) | Deleted with the peer (`ON DELETE CASCADE`) |
| `timeline` | `id` | What moved between devices (History): `kind`, `device_id`, `device_name`, `incoming`, `timestamp`, `title`, `detail`, `target`, `size_bytes`, `duration_secs`, `ref_id` | Kept for the chosen timeline retention (`set_timeline_retention`); can be cleared |
| `meta` | `key` | Key-value settings (the device name, timeline retention…) | Permanent per installation |
| `chat_threads` | `(peer, thread_id)` | Conversations from `MessagingStyle` notifications: `app`, `app_name`, `notification_key`, `reply_action_id`, `title`, `is_group`, `snippet`, `last_sender`, `updated_ms`, `unread`, `active`, `avatar`, `app_icon` | 90 days (`CHAT_RETENTION_SECS`), at most `MAX_CHAT_THREADS_PER_PEER = 200` |
| `chat_messages` | `id` | Messages of a thread: `sender`, `text`, `time_ms`, `self_sent`, `avatar` | At most `MAX_CHAT_MESSAGES_PER_THREAD = 200` per thread, pruned with it |
| `send_queue` | `id` | Persistent outgoing queue: `seq`, `peer_id`, `kind`, `status`, `created_at`, `expires_at`, `next_retry_ms`, `attempts`, `title`, `detail`, `payload_json`, `payload_blob`, `bytes_total`/`bytes_done`, `files_total`/`files_done`, `error_msg` | Active items capped at 200 / 10 GiB; finished ones stay until cleared |

`peers`, `peer_toggles`, `timeline` and `meta` are `STRICT` tables. Migration 8 added `peers.install_id`: a salted hash of the other device's Android ID (or the PC's machine ID), used only to replace the old entry when the same device is paired again (merged by install ID and device kind, never by name). Migration 9 removed what NectarLink's voice recorder left: queued recordings are dropped, their history entries become files, and the `recordings` toggle is forgotten.

## Unified Conversations (`chat_threads` & `chat_messages`)

When an Android notification uses `Notification.MessagingStyle` (WhatsApp, Signal, Telegram, Messages…), the phone's `NotificationReader.kt` extracts the conversation, its messages and the `RemoteInput` reply action.

- `Store::upsert_chat_notification` upserts the thread in `chat_threads` and adds its new messages to `chat_messages`.
- A reply sent from the PC is appended with `self_sent = 1` (`append_chat_reply`) and runs the notification's reply action on the phone.
- Each upsert prunes the thread's messages and the peer's old threads (90 days, 200 threads, 200 messages per thread).

## Persistent Bidirectional Send Queue (`send_queue`)

Implemented across `crates/broche-core/src/store.rs`, `crates/broche-core/src/node.rs`, and `crates/broche-core/src/transfer.rs`.

Whenever a user sends files, folders, links, clipboard payloads, handoff documents, task alerts, or camera requests while the target device is offline (or when the connection drops mid-operation), Broche enqueues the action into `send_queue` and delivers it automatically when a session is established.

### Queue Item Kinds and Default TTLs (`QueueKind`)

| `QueueKind` | Wire Value | Default TTL | Description |
|---|---|---|---|
| `QueueKind::Files` | `"files"` | 7 days (`604,800,000 ms`) | One or more files (`QueuePayload::Files`) |
| `QueueKind::Folder` | `"folder"` | 7 days (`604,800,000 ms`) | Directory tree with relative paths (`QueuePayload::Files { is_folder: true }`) |
| `QueueKind::HandoffDocument` | `"handoff_document"` | 7 days (`604,800,000 ms`) | Document file + reading position (`position_json`) |
| `QueueKind::Clipboard` | `"clipboard"` | 24 hours (`86,400,000 ms`) | Text clipboard payload (`QueuePayload::Clipboard`) |
| `QueueKind::ClipboardImage` | `"clipboard_image"` | 24 hours (`86,400,000 ms`) | Image clipboard payload (`QueuePayload::ClipboardImage`) |
| `QueueKind::Link` | `"link"` | 24 hours (`86,400,000 ms`) | URL + optional title (`QueuePayload::Link`) |
| `QueueKind::TaskNotify` | `"task_notify"` | 24 hours (`86,400,000 ms`) | Long-running command/task completion alert (`QueuePayload::TaskNotify`) |
| `QueueKind::CameraRequest` | `"camera_request"` | 2 minutes (`120,000 ms`) | Interactive remote camera capture request (`QueuePayload::CameraRequest`) |

### Queue States (`QueueStatus`)

```text
           enqueue_item()
                 │
                 ▼
            [ waiting ] ◄─────────────── retry_send_queue_item()
             │       │                   or reset_interrupted_sending()
  TTL passed │       │ peer online &
             │       │ next_retry_ms <= now
             │       ▼
             │  [ sending ]
             │   │       │
             │   │ ok    │ transport drop / integrity error / max attempts
             │   ▼       ▼
             │ (deleted) [ failed ]
             ▼
         [ expired ]
```

- Successfully delivered items are deleted from `send_queue` (`complete_queue_item`) and recorded in `timeline`.
- Failed or expired items remain visible in the UI (`failed` / `expired`) so the user can inspect the error reason, click **Retry** (`retry_queue_item`), or remove them (`remove_queue_item`, `clear_finished_queue_items`).

### Ordering, Replacement, and Capacity Guarantees

1. **Strict Per-Peer FIFO Sequencing**: Every item receives a monotonically increasing `seq` number (`idx_send_queue_peer_fifo` on `(peer_id, status, seq ASC)`). Items for a given peer are drained strictly in `seq` order, while queues for different peers drain independently in parallel.
2. **Clipboard Replacement**: If a peer already has a `waiting` clipboard item (`clipboard` or `clipboard_image`), copying a new clip replaces it and gives it a new `seq` (`MAX(seq) + 1`), so it moves to the end of the queue instead of piling up stale clips.
3. **Capacity Limits**:
   - Maximum active (`waiting` + `sending`) items across the queue: `MAX_QUEUE_ACTIVE_ITEMS = 200`.
   - Maximum active file bytes across the queue: `MAX_QUEUE_ACTIVE_BYTES = 10 GiB` (`10 * 1024 * 1024 * 1024` bytes).
   - Maximum queued clipboard image size: `MAX_QUEUE_CLIPBOARD_IMAGE_BYTES = 50 MiB` in `store.rs` (`clip::MAX_IMAGE_BYTES = 32 MiB` on the wire).
4. **Pre-Send File Integrity Verification (`QueuedFileSpec::verify_integrity`)**: When a file is enqueued, `Store` records its `size` and last-modified timestamp (`mtime_ms`). Immediately before opening the QUIC stream, `verify_integrity` re-stats the path; if the file was deleted, truncated, or modified after queueing, the item transitions to `QueueStatus::Failed` with a descriptive error rather than streaming corrupt or unintended data.
5. **Ephemeral File Descriptor Staging**: On Android, files shared from `content://` URIs may be backed by transient `ContentResolver` file descriptors that close when the share activity finishes. `Node` copies transient inputs into `<data_dir>/queue_staging/<queue_id>/<idx>_<name>` (`QueuedFileSpec::staged`) and deletes the staging directory (`cleanup_staging_id`) as soon as the item completes, is canceled, or is pruned.
6. **Crash Recovery and Zero-Polling Coordinator**:
   - On startup, `Store::reset_interrupted_sending` resets any item left in `'sending'` state by a previous process crash back to `'waiting'` (`next_retry_ms = 0`).
   - The background queue coordinator (`node.rs`) never busy-polls SQLite: it computes the exact next wakeup deadline (`Store::next_queue_wake_ms`) across pending retry timers and TTL expirations, and awaits `std::future::pending()` when the queue is empty until woken by `queue_notify` (triggered on enqueue, retry, or peer session connection).

## Encrypted Clipboard History (`clipboard_history.rs`)

Recent clipboard items are persisted in `<data_dir>/clipboard_history.enc` and `<data_dir>/clipboard_images/<id>.enc`:

- **At-Rest Protection (`KeyProtector`)**: Both the index (`clipboard_history.enc`) and each image file (`clipboard_images/<id>.enc`) are sealed with the platform `KeyProtector` (the Linux keyring protector, or the Android Keystore).
- **Atomic Writes**: Updates write to `.tmp` files (`clipboard_history.enc.tmp` and `<id>.enc.tmp`) and atomically rename them into place.
- **Retention Limits**:
  - Up to `MAX_CLIPBOARD_HISTORY = 50` entries.
  - Automatic 7-day expiration (`MAX_CLIPBOARD_AGE_SECS = 7 * 86_400`).
  - Maximum single image size: `MAX_IMAGE_BYTES = 16 MiB`; maximum total image size: `MAX_IMAGES_TOTAL = 128 MiB`.
- **Sensitive Content Filtering**: one-time codes (`otp::is_otp_clip`) are never stored. Sensitive clips never reach the core: the Linux app skips copies marked `x-kde-passwordManagerHint` (what password managers set) and Android skips clips marked `ClipDescription.EXTRA_IS_SENSITIVE`.
