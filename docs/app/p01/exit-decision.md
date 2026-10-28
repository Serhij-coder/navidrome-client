# P01 sequencing decision

Status: accepted by the maintainer on 2026-10-28 for bounded P02.01-P02.04 implementation. P00/P01 remain open; this sequencing exception does not waive evidence requirements or authorize release/parity claims.

P01 has expanded through repeated feasibility increments without an exit decision. The project has substantial prototype evidence but no production domain/persistence/backend/queue implementation and no usable music client. Adding more independent prototypes is not the shortest path to an application. The remaining evidence must be tracked separately from a decision about which work can proceed safely now.

## Current gate audit

| Task | Demonstrated | Remaining gate |
| --- | --- | --- |
| P01.01 | Pinned Cargo workspace/toolchain/lock, explicit tests, formatting/lint/license metadata | Complete for workspace establishment |
| P01.02 | Native egui Glow/wgpu candidates and independent web/remote WASM roots | Physical renderer/driver/platform comparison; production renderer decision |
| P01.03 | Virtual 100k rows, selection/reorder, bounded metadata/artwork workers, synthetic/concurrent tests | Calibrated presented-frame/input/CPU/GPU/RSS/idle/retention budgets on declared hardware |
| P01.04 | Lab browser playback, WebAudio/session controls, disposable server/codec/CORS/lifecycle fixtures | Full live authenticated stream policy, device/current-browser/mobile matrix and recorded seek boundaries |
| P01.05 | Managed mpv IPC/lifecycle, control ownership, Linux MPRIS subset, crossfade/filter/waveform probes and GStreamer candidate; prepared local replacement has local checks and two completed reviews, with its new hosted result still unverified | Accepted replacement/fallback strategy in ADR-003; representative engine/device/DSP and native target evidence or explicitly accepted remediation. Full production queue/authentication handoff remains P06 work |
| P01.06 | Keyboard/context/clipboard, toolkit IME/scaling regressions, Linux Orca journey, browser input, both AppImage CPU jobs | Full physical IME/RTL/font/DPI/touch/accessibility/theme/platform matrix and measured renderer acceptance |
| P01.07 | Tracked-input native/WASM/tooling CI and artifact smoke, including seven successful hosted jobs | Complete for CI establishment; new revisions and physical/release acceptance remain separate |
| P01.08 | Bounded original spectrum/waveform, owned Linux monitor capture and lifecycle/disposal tests | Actual upstream preset strategy, physical cost, platform capture/permission and consumer/engine handoff matrix |

Detailed evidence and limits remain in the [P01 matrix](README.md), [initial verification](verification.md), [playback ADR](../adr/003-playback-boundaries.md), [visualization ADR](../adr/008-visualization-feasibility.md), [GStreamer record](gstreamer-candidate.md) and [prepared handoff](prepared-handoff.md). A prototype or passing build does not establish parity. P00.07 calibration is a direct remaining P01 performance requirement. Complete native/plugin source obligations are contingent on engine adoption/bundling and release packaging; they are not a technical dependency of portable P02 models.

Real server/library artwork churn, full adapter/queue integration, production engine/session restoration, bulk screens and complete release operations belong to P03-P13. They must stay tracked there rather than continually extending a bounded P01 prototype. P01 still needs the feasibility decisions and evidence actually specified in its gate, including explicitly accepted remediation where allowed; its existing ADRs do not currently record those open cases as accepted exit remediation.

## Authorized sequencing decision

The maintainer authorized starting bounded P02 portable foundation work while keeping P01 open. This is the same kind of explicit sequencing exception previously given for P01 ahead of P00, not a claim that P01 has passed. The local rewrite plan lists P02's dependency as P01; the maintained [P01 next-work rule](README.md#troubleshooting-and-next-work) formerly said "Continue only P01". This recorded authorization supersedes that sequencing restriction only for the bounded work described here.

The first P02 deliverable should be portable normalized models, distinct IDs/time units and typed command/event/effect and adapter contracts (P02.01-.03), checked on native and WASM. The next deliverable is isolated, versioned non-secret profile/preferences persistence with corruption/recovery fixtures and a secret-store boundary (bounded P02.04). Historical imports and full platform credential remembering retain P02.05/.06 characterization and validation requirements. These boundaries do not require choosing the final renderer, fallback engine, audio device or visualizer implementation.

Then pursue the already-planned bounded development demonstration: one Navidrome login/list/queue/play-pause-seek path using managed mpv. It must be labeled a partial vertical slice, retain other backends/browser/remote requirements in the migration map and avoid implying that adapter, queue or P06 phase exits are complete. This is the intended route toward a usable client, rather than more standalone P01 probes.

P01 work during this assignment resolves only a concrete blocker encountered by the authorized implementation. Physical/platform checks, fallback adoption and release licensing stay named requirements; none becomes passed, removed or silently deferred. Bulk screen expansion and release/parity claims retain their existing gates. P01 issues are addressed during this assignment only when they concretely block the authorized implementation. Its remaining evidence is retained separately.

Authorized now: coherent P02.01-P02.03 foundations followed by bounded P02.04 non-secret persistence. P02.05 historical imports/rollback, P02.06 credential remembering and P02.07 cache/cancellation remain tracked follow-up work. The later partial Navidrome vertical slice is the intended next direction, not a phase exit or permission to omit other deliverables.
