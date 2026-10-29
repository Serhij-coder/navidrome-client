# P01.05 prepared local-source replacement

`HandoffSession::replace_prepared` extends the [serialized replacement probe](native-handoff.md) with caller-supplied local source, position in seconds, pause/play intent and volume percentage. It returns a control client only after preparation is observed. The existing `replace` method still installs an endpoint without promising readiness. Neither API selects a production fallback, owns a queue, snapshots the previous engine or restores authentication, capture or OS integration.

## Preparation and publication

One supervisor retains the existing one-slot replacement queue and one active control worker. A successfully admitted generation is consumed even if the request later fails, expires or is cancelled. Paths are bounded to 4096 Unicode bytes and must be nonempty without NUL; position must be finite/nonnegative and volume finite within 0..100. Validation does not access the filesystem on the caller thread. The owning control worker canonicalizes and validates the local file.

The supervisor retires the previous endpoint and waits for cleanup before starting a candidate. It then:

1. Pauses the candidate before loading the local source.
2. Observes a position within 0.1 seconds of zero rather than treating the load acknowledgement as readiness.
3. Issues one absolute seek and observes the requested position within 0.1 seconds, including position zero.
4. Sets volume and reads it back within 0.001 percentage points.
5. Applies the requested pause/play state and verifies observed playback status before publishing the client.

An out-of-duration target is an explicit `Unsupported` preparation failure; it does not become a successful seek no-op or an indefinite wait. Wrong volume/status or unexpected typed replies fail as protocol errors. Read queries may be polled; mutations are not retried. A successful playing endpoint may advance after the verified seek. Volume preserves the requested engine percentage, not measured acoustic gain: mpv and GStreamer have different gain behavior, as the [crossfade record](crossfade.md) already distinguishes.

The source/settings are explicit intent supplied by the caller. This experiment does not read old engine state into a snapshot or identify a queue occurrence/server/library item. A future application owner must capture and correlate that state deliberately. The [GStreamer candidate](gstreamer-candidate.md) and mpv share the managed control boundary in the real cross-engine probe, but automatic selection and production source handoff remain open decisions.

## Deadlines, cancellation and failures

The caller supplies a positive budget, capped at 15 seconds from admission. One absolute deadline includes queueing, old-engine retirement, startup and all preparation. Each routed command also retains the existing three-second cap. A slow startup may therefore fail preparation even while the startup worker still owns its longer existing startup sequence. No existing command budget was widened.

Before retirement, cancellation or an expired queued request can preserve the active endpoint. After retirement, failure cannot revive it. Cancelled, expired, dropped or failed candidates are shut down and cleanup is confirmed before another replacement is attempted. Shutdown bypasses queue pressure. Polling checks cancellation/shutdown/deadline between operations and while awaiting replies. An issued mutation can already have executed; completion can win a concurrent cancellation at publication.

Cleanup retains the existing separate seven-second confirmation budget, including after the caller's preparation deadline expires. It may wait for an in-flight control operation. Kernel scheduling, filesystem work and process kill/reap are outside hard realtime guarantees. Any cleanup error stops admission before its failure reply is delivered. The control completion API still cannot distinguish every startup failure from unconfirmed cleanup, so a missing-binary startup can conservatively require a fresh supervisor. This increment does not silently weaken that guard to implement fallback.

## Source evidence and deliberate boundary

The immutable baseline is `7d6ad318c79802e226996cfc39acba6b05bb22e8`. Stable IDs/dispositions are recorded in the [P01 map](../migration/p01-map.csv). Static source inspection found:

- `player-config.tsx:245:31` disables manual engine selection during playback/casting; its `:274:27` callback persists engine type without a source snapshot. Its `:290:30` reload callback stops media before reload.
- `mpv-player-engine.tsx:100:31` repopulates current/next sources and uses fixed 400/800 ms delays around initialization/pending positive handoff seek (source lines 169-190). The main restart callback `src/main/features/core/player/index.ts:331:5` does not await old quit before recreation.
- DLNA selection/disconnect callbacks at `dlna-cast-button.tsx:149:9` and `:318:42` pass timestamps and restore engine/volume. Queue/device URI and transport-state reconciliation is separate. `player-handoff.ts` does not contain a general local source/occurrence snapshot, and pending local seek is only recorded for positive positions.
- AudioBook/general resume hooks (`use-resume-position.ts:8:34`, `use-queue-restore.ts:47:43`) use occurrence/status guards and fixed delays, not engine readiness.

These are source observations, not new runtime certification. The prepared probe deliberately waits for observed state and verifies zero position. It does not claim to port DLNA, queue restore, AudioBook policy, current/next semantics or the upstream UI. [ADR-003](../adr/003-playback-boundaries.md) records the experimental ownership/readiness policy; production contracts remain P02/P05/P06 work after P01 gates.

## Verification and reproduction

The explicit [prepared_handoff target](../../../tests/native-spike/prepared_handoff.rs) uses a tracked [Unix peer](../../../tests/native-spike/prepared_handoff_peer.py) with file gates for readiness and seek arrival. Protocol fixtures use disposable local files and do not decode audio. Eight default tests cover publication ordering, one seek mutation, zero position, readback faults, exact preparation failures, cleanup/no process overlap, explicit retry, stale/retired clients, cancellation/drop/shutdown, queue pressure, admission validation and expiration while queued.

Two ignored real probes generate an owned eight-second tone and use null output. The mpv probe performs four prepared replacements; the optional GStreamer probe performs mpv -> GStreamer -> mpv. Both verify requested state/position/volume and retirement of old clients, with explicit cleanup after assertion failures. Playing position assertions account for elapsed playback time; paused position stays within 0.1 seconds. These are control/process observations, not device timing or audible continuity.

```sh
cargo test -p feishin-native-spike --test prepared_handoff --locked
python3 tools/app/test-native.py --prepared-handoff
python3 tools/app/test-native.py --gstreamer-handoff
```

The real modes require external mpv; the second also requires the optional GStreamer development/runtime packages from the [candidate guide](gstreamer-candidate.md). `P01_MPV` may name a trusted executable. Modes are mutually exclusive with all other wrapper modes and preserve failures/identifier-only CI annotations. Both Linux AppImage jobs invoke these probes; the existing wrapper modes keep their original command scopes. Native Windows compilation is separate from real prepared-mpv execution on Windows. No new dependency or AppImage payload is added.

The [increment manifest](evidence/prepared-handoff-manifest.json) records exact checks and reviews. No P01 exit or production fallback adoption is claimed. Remaining work includes source identity/authentication and application policy, native/device/platform support, capture/OS consumer handoff, calibrated performance and distribution licensing.

Both independent reviewers found no actionable findings in the implementation and sequencing proposal. The operations reviewer independently passed both real-engine probes. Exact checks are in the manifest; the proposal still requires the maintainer decision before P02 begins.

## CI deadline classification correction

The P03.01 merge CI [run 40880607371](https://github.com/Serhij-coder/navidrome-client/actions/runs/40880607371) failed the strict no-arrival deadline assertion in `preparation_failures_retire_candidate_before_explicit_retry`. The original assertion did not print the actual category. The same committed tree passed all 220 tests in a clean worktree, so the hosted failure itself was not locally reproduced.

Inspection found a concrete classification gap: preparation could expose `ControlFailure::Engine(Error::Deadline)` as a nested engine error while an outer control deadline became `HandoffFailure::Deadline`. Both now become the handoff deadline category, preserving other engine errors and cancellation/stopped outcomes. A private deterministic regression fails on the committed pre-fix classifier and passes with this correction. The no-arrival assertion remains strict and now reports its error category. This addresses the evidenced gap without asserting that the unprinted CI category is proven. Focused native tests passed (16 passed/four optional ignored); the final independent Sol medium review found no actionable findings and independently passed the classifier regression. Fresh hosted CI is still required. This is a concrete implementation blocker correction, not a new P01 prototype or exit claim.
