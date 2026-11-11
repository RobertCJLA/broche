# Protocol Overview

The Broche wire protocol (`broche/0`) runs over a single mutually authenticated QUIC + TLS 1.3 connection per peer (`iroh` `1.3`). It multiplexes control messages, request/response RPCs, bulk file streams, real-time media streams, and low-latency datagrams without head-of-line blocking between independent features. This page is a map; the normative text is in [`docs/protocol/`](../protocol/v0.md).

## Specification Index (`docs/protocol/`)

| Specification | Capability IDs | Summary |
|---|---|---|
| [`v0.md`](../protocol/v0.md) | `core.ping`, `device.battery`, `device.ring`, `camera.capture`, `camera.continuity` | Transport, ALPNs (`broche/0`, `broche-pair/0`), framing, `hello` / `hello.update`, pairing (QR, nearby, USB), error codes, Continuity Camera, LocalSend interop |
| [`capabilities.md`](../protocol/capabilities.md) | all | The registry of capability IDs and what unlocks each |
| [`actions.md`](../protocol/actions.md) | `link.open`, `task.notify`, `pc.power`, `pc.wake` | Opening links, task notifications, lock/sleep/shutdown, Wake-on-LAN |
| [`calls.md`](../protocol/calls.md) | `call.state`, `call.control`, `call.incall`, `call.log`, `call.dial`, `call.show` | Live calls, answer/decline, in-call controls, call log and dialling |
| [`clipboard.md`](../protocol/clipboard.md) | `clip.image`, `clip.write` | Text clips (`clip.set`) and images on their own stream |
| [`contacts.md`](../protocol/contacts.md) | `contacts.read`, `contacts.show` | The phone's contacts for the PC |
| [`deck.md`](../protocol/deck.md) | `deck.actions`, `pc.audio` | The phone's PC shortcuts: layout, state, button presses and the PC's volume |
| [`files.md`](../protocol/files.md) | `files.transfer` | Files and folders with resume, pause and cancel |
| [`media.md`](../protocol/media.md) | `media.control`, `media.remote` | What plays on one device, controlled from the other |
| [`mirror.md`](../protocol/mirror.md) | `mirror.view`, `mirror.listen`, `mirror.capture`, `mirror.virtual_display` | The phone's screen and sound on the PC, the PC's screen on the phone, phone apps in windows |
| [`notifications.md`](../protocol/notifications.md) | `notify.mirror`, `notify.live` | Notifications, replies, actions and conversations |
| [`photos.md`](../protocol/photos.md) | `photos.read`, `photos.show` | Recent photos, thumbnails and full files |
| [`remote.md`](../protocol/remote.md) | `input.inject` | Presentation remote, trackpad and keyboard on the PC |
| [`sms.md`](../protocol/sms.md) | `sms.read`, `sms.send`, `sms.show` | Text message threads, history and sending |
| [`storage.md`](../protocol/storage.md) | `storage.read`, `storage.write`, `storage.mount` | Browsing the other device's shared storage |
| [`toggles.md`](../protocol/toggles.md) | `toggles.read`, `toggles.ringer`, `toggles.volume`, `toggles.flashlight`, `toggles.dnd`, `toggles.show` (and `toggles.wifi`, `toggles.bluetooth`, not offered by Broche's Android app) | The phone's quick settings |
| [`webcam.md`](../protocol/webcam.md) | `camera.stream`, `camera.virtual` | The phone as the PC's camera |

## Framing (`crates/broche-protocol/src/frame.rs`)

Every frame is an unsigned LEB128 length (1 to 4 bytes) followed by a CBOR payload, at most 1 MiB (`MAX_FRAME_LEN`); a larger one fails with `FRAME_TOO_LARGE`. The payload is an envelope `{ t, id?, re?, b? }` (v0 §3): the message type, a request ID, the ID it answers, and the body. Unknown keys are ignored, so minor revisions can add optional fields.

## Streams and Datagrams

| Primitive | Opened by | Use |
|---|---|---|
| **Control stream** | The dialing device, first | `hello`, `hello.update`, `ping`, events and most short requests |
| **RPC / service streams** | Either device | One operation; starts with a `stream` header `{ svc, op, v }` (for example `files`/`send`/`1`) |
| **Bulk streams** | Either device | After their header frame, raw bytes or video packets until the end |
| **Datagrams** | Either device | Loss-tolerant, latency-critical data (pointer motion, short sound frames) |

## Handshake and Capabilities

Both devices send `hello` on the control stream (`proto`, `min`, `app`, `device`, `caps`, `power`; v0 §5). A feature works when the other device announces the capability it needs and both devices' toggles allow it. When something changes (a permission granted on the phone, the power level, the device name), a device sends `hello.update` without reconnecting. `ping` measures the round trip alongside QUIC's 1-second keep-alive.

## Files (`files.transfer`)

Implemented in `crates/broche-core/src/transfer.rs`, specified in [files.md](../protocol/files.md). The sender offers the files (name, size, optional folder and BLAKE3 digest, `FileEntry`); the receiver answers with how much of each it already has, so an interrupted transfer resumes from the byte where it stopped, and checks the digest before moving the file out of `<data_dir>/incoming/`. Data goes in 256 KiB chunks and progress is published at most every 200 ms (`PROGRESS_EVERY`). Pausing or cancelling closes the stream with `files::PAUSED` (11) or `files::CANCELLED` (10). Sends to an offline device wait in the persistent send queue ([Storage and Retention](storage-and-retention.md)).

## Video and Sound

Specified in [mirror.md](../protocol/mirror.md) and [webcam.md](../protocol/webcam.md); packets in `crates/broche-protocol/src/video.rs`. After its header frame, a video stream is a series of packets: a 4-byte big-endian length, a 1-byte kind (`PacketKind`: codec config, frame or keyframe), an 8-byte big-endian timestamp in microseconds, then the codec data; a packet is at most 8 MiB (`MAX_VIDEO_PACKET`). Short sound frames (at most 960 bytes of PCM) may go as datagrams tagged `NLPA` with their sample rate and channel count; otherwise sound uses a stream.
