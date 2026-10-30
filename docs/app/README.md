# Application documentation

This directory contains maintained documentation for the Rust application. The root [README](../../README.md) is the public introduction; [AGENTS.md](../../AGENTS.md) contains the development rules. The project uses the GNU GPL version 3 license in [LICENSE](../../LICENSE).

Start with the brief [project scratchpad](STATUS.md) for current state, checks, blockers and next action. Keep detailed setup, behavior and design documentation here; the scratchpad is limited to 25 non-empty lines and is not a running history.

The desktop UI direction is egui, with native desktop performance and usability as priorities. Layout and appearance may differ from Feishin while retaining agreed features, effective workflows, accessibility and localization. Native audio, visualization, capture and browser/remote boundaries need implementation evidence; toolkit selection is not a measured performance result.

P01 now has a pinned Rust workspace and runnable native egui, full-web and remote feasibility shells. The [P01 guide](p01/README.md) contains setup/build/run/test, architecture boundaries, packaging, privacy and troubleshooting; the [verification record](p01/verification.md) separates demonstrated results from open gates. The [continuation](p01/continuation.md) adds input/media/capture lifecycle and concurrent/native candidate evidence, plus initial hosted CI outcomes. This is not a working music client. The [Orca continuation](p01/screen-reader.md) adds native selection announcements and a real screen-reader event journey; [58814c1 CI evidence](p01/evidence/hosted-windows-ipc.json) completes P01.07 establishment. The [browser seek comparison](p01/browser-seeking.md) corrects the forced-loop fixture and records remaining boundary failures. The [seek lifecycle remediation](p01/seek-deadlines.md) adds cancellable Media Session seek deadlines and explicit failure/reload behavior. The [native transition probe](p01/native-transitions.md) records captured energy/prefetch experiments separately from sample-exact/device certification. The [element-seek continuation](p01/native-control-seeking.md) shares cancellation/deadlines across browser control paths. The [dual-mpv crossfade experiment](p01/crossfade.md) separates source coefficients, volume conversion and captured overlap from production audio parity. The [native position probe](p01/native-position.md) separates readiness and observed paused seek arrival from command acceptance. The [keyboard track actions](p01/track-actions.md) add focused copy/cancel/focus restoration with native clipboard and repeated Orca journeys. P02 now has [portable domain foundations and bounded non-secret native/browser persistence](p02/README.md) under the maintainer's sequencing exception; P00/P01 remain open.

P00 now has tracked [baseline evidence and open gates](baseline/README.md): route/composition analysis, 52 source-anchored target journeys, synthetic fixtures and [controlled reference builds](baseline/reference-run.md). The 2026-10-22 increment adds [six live old/current server fixtures](baseline/live-servers.md), [headless queue/persistence/scrobble traces](baseline/headless-characterization.md), raw 10k/100k queue timings and font notice evidence. The subsequent [browser/desktop/remote run](baseline/browser-characterization.md) adds 51 original UI screenshots and selected real input, storage, media and live-server flows. Full journeys, adapter/engine integration, live large libraries, audio/device behavior and calibrated desktop measurements remain incomplete. The maintainer explicitly authorized starting P01 prototypes while these P00 gates remain open; neither phase exit is claimed. Run the tracked evidence/tooling checks documented in the baseline guide; optional reference checks are separate.

The subsequent [adapter characterization](baseline/adapter-characterization.md) adds 54 original controller/client/interceptor/normalizer replay cases, following saved server responses and callback arguments through an offline Axios boundary. It records partial adapter behavior and payloads separately from live authenticated application traffic.

The native-control/input increment adds [one bounded mpv control owner and an opt-in private-bus MPRIS adapter](p01/native-controls.md), [composition/scaling regressions and remediation](p01/text-input.md), [full-web/remote pointer and touch journeys](p01/browser-input.md), and [native AppImage file/package provenance](licenses/native-artifacts.md). These remain feasibility results with explicit platform, input, engine and distribution limits; no additional phase exit is claimed.

Keep application test code and fixtures in the dedicated top-level [tests directory](../../tests/README.md), including tests of app tooling. P01 wires Rust files into Cargo test targets/private test modules and configures CI to execute them; their location alone does not make them run.

The [native replacement continuation](p01/native-handoff.md) adds serialized retirement/replacement with generation rejection and bounded cleanup. It separates endpoint installation from readiness, documents the upstream fallback-notification source finding, and keeps alternative-engine selection and the P01 gate open.

The [GStreamer candidate](p01/gstreamer-candidate.md) adds an opt-in isolated native helper and a 12-case local/authenticated-loopback codec comparison with mpv. It demonstrates controls and cleanup with null output; no production fallback or additional phase exit is selected.

The [prepared source handoff](p01/prepared-handoff.md) verifies explicit local position, pause/play and volume before publishing a replacement control client. Real mpv and cross-engine null-output probes complement the endpoint-only replacement experiment; automatic fallback and production state/consumer restoration remain open.

Maintained ADRs record [egui direction and P01 criteria](adr/001-egui-desktop.md), [target matrix](adr/002-target-matrix.md), [playback boundaries](adr/003-playback-boundaries.md), [source differences](adr/009-source-differences.md) and [development identity/privacy/update policy](adr/010-development-policy.md). P00 [migration anchors](migration/p00-map.csv) remain unmapped; the separate [P01 map](migration/p01-map.csv) records prototype boundaries and deferred production behavior. No full feature port is claimed. [ADR-008](adr/008-visualization-feasibility.md) records bounded visualization/capture evidence and remediation. [Performance budgets/protocol](baseline/performance.md) distinguish uncalibrated numerical candidates from unavailable runtime results, including the provisional p95 <=16.7 ms at 60 Hz for 100k-row scrolling.

Update app documentation, licenses/notices and relevant operational instructions in the same task as behavior or configuration changes. Keep required build/test instructions reproducible from tracked files. Local rewrite plans, inventory, reference archives and analysis tools outside `docs/app/` and `tools/app/` are ignored development references; application builds and CI must not depend on their presence.

P02 decisions are recorded in [ADR-004](adr/004-persistence.md) and [ADR-005](adr/005-portable-contracts.md); separate [foundation](migration/p02-foundation.csv) and [storage](migration/p02-storage.csv) maps retain source evidence and unfinished imports/credentials/cache work.

The bounded [P02.05 import continuation](p02/imports.md) adds copied-input preview and a separate durable [apply/backup/rollback journal](p02/import-journal.md). It preserves source copies and explicitly reports unported settings/transforms; shell adoption and full historical compatibility remain open.

The [retained historical settings continuation](p02/settings-history.md) covers settings versions 14-34 for columns and supplied Auto DJ objects, using captured original migration outputs. Full settings hydration/adoption and phase exits remain open.

The [early settings continuation](p02/early-settings.md) covers versions 0-13, including explicit retained-settings reset previews for 10/11, preserved profile/queue state and journal undo. Full state adoption and phase exits remain separate.

The [active-state continuation](p02/state-adoption.md) adds a single durable repository for reviewed stage adoption, local saves and undo/redo, with v1 profile compatibility. Production shell/worker composition and phase exits remain open. Current build/test scope is Linux and WASM/browser; Windows and macOS runs are excluded per the maintainer's instruction in [ADR-002](adr/002-target-matrix.md).

The [storage worker continuation](p02/state-worker.md) adds typed local session dispatch and a bounded native storage thread with review token ownership, correlation and latest-start cancellation. UI/shell composition and in-flight browser/native I/O cancellation remain open.

The [opt-in desktop preferences](p02/desktop-preferences.md) now consume the isolated worker for two preference fields. This native flow preserves imported durable data and keeps the synthetic table independent; full profiles/import/browser/playback integration remains open.

The [credential library](p02/credentials.md) adds bounded session-only defaults, explicit retention operations and an optional Linux Secret Service adapter, tested on an owned GNOME provider. Browser remembering is explicitly unsupported; application authentication/consent and provider-prompt integration remain open. [ADR-006](adr/006-credentials.md) records deliberate differences from upstream retention.

The [query cache foundation](p02/query-cache.md) supplies bounded server/library-scoped keys, correlated tickets and scoped invalidation without runtime backend/UI wiring. [ADR-007](adr/007-query-cache.md), [migration records](migration/p02-cache.csv) and [verification](p02/query-cache-verification.md) distinguish implemented rules from remaining P02.07 serialization/integration work. The separately developed [credential executor](p02/credential-executor.md) now supplies bounded native scheduling; consent and provider prompt limits remain open.

The [P03.01 Subsonic foundation](p03/README.md) implements portable URL/subpath/query construction, salted or explicit legacy parameters and redacted wire values under a bounded maintainer-authorized sequencing exception while P02 remains open. Exact pinned-source replay and [verification](p03/verification.md) cover construction only; no HTTP login, library operation or P02/P03 phase exit is claimed.

The [P02.07 serialization continuation](p02/serialization.md) enforces existing durable snapshot byte limits during output construction. The parallel [P03.02 protocol foundation](p03/protocol.md) adds envelope/errors, conditional discovery and server-bound form planning with cooperative local transport checks. Live transport and phase exits remain open.

The [P02.07 payload continuation](p02/payloads.md) adds bounded normalized song-summary page and retained-queue handoff codecs with exact cache accounting. The source remote protocol is a separate unported contract.

The bounded [P03.03 song detail read](p03/song-reads.md) normalizes `getSong` through the existing local transport, with pinned-source replay and checked IDs/units. Other reads, production HTTP and phase exits remain open.

The [P03.03 album detail continuation](p03/album-reads.md) adds album and album-artist reads, preserving optional albums and nested disc/song behavior under a bounded work budget. Other operations, real transport and phase exits remain open.

The [P03.03 playlist snapshot reads](p03/playlist-reads.md) preserve metadata, ordered duplicate entries and song-ID projections. The maintainer requested continuous progression through the rewrite plan; required reviews, evidence gates and release/deployment authorization remain in force.

The [P03.03 collection reads](p03/collection-reads.md) add music folders, artist roles and random/similar songs with explicit source folder/filter limits and deterministic original-controller evidence.

The [P03.03/P03.04 local lists](p03/local-lists.md) add playlist lists/counts and genre lists with source search/sort/slicing, including null-order and zero-limit characterization.

The [P03.03 enrichment reads](p03/enrichment-reads.md) add artist information and album/artist recommendations while retaining duplicate songs, optional metadata and source-specific HTTP behavior.

The [P03.03/P03.04 artist lists](p03/artist-lists.md) add artist/album-artist list/count flows with source filter timing, related metadata and characterized ineffective sorts.

The [P03.04 album counts](p03/album-counts.md) add bounded source-specific pagination/header/fan-out replacement, explicit calendar samples and error/context evidence. No generic count or phase exit is claimed.

The [P03.03/P03.04 album lists](p03/album-lists.md) add all source list branches, normalized nested data and deterministic local ordering with characterized date differences.
