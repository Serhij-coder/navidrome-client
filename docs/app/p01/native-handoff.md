# P01.05 serialized native engine replacement

This experiment adds [HandoffSession](../../../crates/native-spike/src/handoff.rs) above the existing managed [control worker](native-controls.md). One supervisor retires the old control session, waits for cleanup confirmation, then starts the next session. It does not restore a source, queue, position, volume, capture session or OS media registration. It does not choose or automatically invoke a fallback engine. P01.05 and the overall P01 exit remain open.

## Ownership and completion

Construction, replacement admission and Drop do not wait for IPC or child cleanup. The supervisor has one in-progress replacement and at most one queued replacement; every request has a one-slot reply. Each accepted request consumes a strictly increasing generation, including requests later cancelled. Invalid inputs and queue-full submissions do not consume a generation. Generation zero is valid; after accepting `u64::MAX`, a new supervisor is required for further replacements.

Old `ControlClient` clones stop accepting commands before a replacement worker starts. An old reply already delivered retains its old generation/request identity; a consumer must still reject stale replies. This is an experiment in engine ownership, not a production queue reducer or event coordinator. Success installs a new control endpoint. Engine startup can subsequently fail; a successful handoff reply does not mean a child has started, a source is ready or audio has reached a device.

Cancellation before retirement may preserve the old session. After retirement begins, cancellation cannot revive it. The supervisor checks cancellation/shutdown again after starting a candidate and retires an abandoned candidate before processing another replacement. Cancellation after a reply is delivered does not undo that installed endpoint. A caller's blocking `wait` timeout does not cancel its request; use `cancel` or drop the request. UI code must poll `try_reply`.

Shutdown bypasses the queue. Each retirement waits up to seven seconds for the control worker, whose existing startup comprises up to two three-second exchanges plus cleanup. This is a protocol budget, not a hard bound on OS scheduling or process kill/reap. Any cleanup error permanently stops replacement admission. After an error, a timed-out worker may still own cleanup; the supervisor never starts another child. The underlying completion API does not distinguish a missing-binary startup failure from an unconfirmed cleanup failure, so even the former conservatively requires a fresh supervisor. No automatic retry is claimed.

## Source behavior and fallback candidates

The pinned upstream is `7d6ad318c79802e226996cfc39acba6b05bb22e8`. Stable function IDs and dispositions are in the [P01 migration map](../migration/p01-map.csv). The local archive and inventory remain separate baseline evidence.

The main-process `createMpv` catches `mpv.start()` failure at `src/main/features/core/player/index.ts:179-185`, then continues initialization. The `player-initialize` handler at lines 364-383 catches errors and calls `setAudioPlayerFallback(true)`; the stable function is `index.ts:305:32`. It emits `renderer-player-fallback`. Its renderer subscriber at `src/renderer/features/player/components/audio-players.tsx:198-210` logs the notification. The component's engine selection at lines 375-399 follows configured `PlayerType`. Static inspection found no consumer of this notification that changes that type. The wording "falling back to web" is therefore not proof of an automatic switch or continued playback. This is source characterization, not a reproduced upstream runtime failure or an adopted behavioral change.

The upstream restart handler at main-player lines 331-359 requests quit without awaiting it before recreation. The Rust probe deliberately requires cleanup confirmation before replacement, following [ADR-003](../adr/003-playback-boundaries.md) ownership requirements. It does not claim restart timing parity. Upstream DLNA timestamp handoff is a separate workflow and remains outside this local-engine experiment.

| Candidate | Evidence available | Work still needed before selection |
| --- | --- | --- |
| Managed mpv | Local-file IPC, current/next, observed seek, control ownership and this serialized replacement probe | Authenticated streams, full device/DSP/OS behavior and supported-platform runtime matrix |
| ffplay/SDL | Existing initial-seek, dummy-output decode and process-lifetime probe | No command IPC in the current candidate; dynamic pause/seek/device/authenticated handoff remain unsupported |
| GStreamer playbin | A possible controllable alternative; host runtime tools/libraries were found during read-only investigation | No Rust integration, development-library/build verification, measured playback matrix or plugin/license packaging audit |
| Embedded libmpv | A possible existing engine boundary; no implementation in this workspace | Library availability, safe resource/FFI boundary, licensing/distribution and target evidence; embedding alone does not demonstrate an independent missing-mpv fallback |

The next fallback experiment should compare one controllable alternative against managed mpv using owned WAV/FLAC/MP3 and loopback-authenticated HTTP fixtures. It must observe pause/resume, seek arrival, end/current-next behavior, failure cleanup, device/gain capability and resource cost, then verify native/plugin notices and clean packaging. These are planned checks, not current results. No dependency was added for an unmeasured candidate.

## Reproduction and evidence

The explicit [Cargo target](../../../tests/native-spike/handoff.rs) uses an owned Unix [IPC peer](../../../tests/native-spike/handoff_peer.py). Startup gates coordinate cancellation and queue saturation; PID checks detect overlapping peers and trace quit/start ordering. The suite covers stale and maximum generations, invalid bounded paths, cancelled/dropped requests, old pending work, shutdown bypass, stalled quit, process death, and missing-binary failure followed by fresh-supervisor recovery. These fixture checks run on Unix; Windows compilation is separate from Windows runtime evidence.

```sh
cargo test -p feishin-native-spike --test handoff --locked
cargo test -p feishin-native-spike --test handoff \
  real_repeated_handoff_paused_readiness_seek_and_retired_controls \
  --locked -- --ignored --exact
```

The ignored probe needs installed mpv (`P01_MPV` may name a trusted executable). It generates an owned local tone and performs four replacements using null output, pausing before each load and observing readiness and seek arrival. Cleanup is awaited even after an assertion panic. CI invokes it in both Linux AppImage architecture jobs. Neither a null sink nor an IPC position establishes audible/device output, gapless transitions, capture handoff or fallback parity. The [increment manifest](evidence/native-handoff-manifest.json) records actual checks, source hashes, reviews and remaining limitations; the [previous commit's hosted result](evidence/hosted-controls-input.json) is independent of this increment.

Implementation and lifecycle tests were assigned to separate implementers with exclusive files in the shared checkout. The orchestrator owns wiring, documentation, combined checks, two independent final reviews and Git delivery. Read-only fallback research supplied source anchors and candidate limitations.

Hosted follow-up: the [96a8a6d native probe step](native-probe-ci.md) failed on x86_64 before packaging while the other six jobs passed. Public evidence does not identify the failing probe; the local 30-invocation reproduction passed. The new CI wrapper reports failed test identifiers, but the underlying hosted failure remains unresolved pending its assertion or a reproduction. The earlier local checks and reviews are historical evidence, not a claim of current all-green hosted CI.
