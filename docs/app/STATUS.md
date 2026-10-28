# Project scratchpad

- Stage: P02.01-P02.03 portable foundations and bounded P02.04 non-secret persistence implemented and reviewed; delivery ready. This is not a usable music client; P00/P01/P02 exits remain open.
- Sequencing: maintainer authorized [bounded P02 while P01 stays open](p01/exit-decision.md) on 2026-10-28; P01 issues only when they concretely block this implementation. No evidence or release/parity gate waived.
- Domain: dependency-free models/catalogs, distinct scoped IDs, checked times, private bounded example owner, correlation/error/capability and !Send service contracts; no P05 reducer or backend/engine port claimed.
- Persistence: schema-v1 isolated native atomic file and browser single-key adapters; non-secret profile subset and two preferences, defaults/partial recovery/future-schema protection. Not wired into shells; native calls require worker composition.
- Evidence: [P02 guide](p02/README.md), [verification](p02/verification.md), [foundation map](migration/p02-foundation.csv), [storage map](migration/p02-storage.csv), [ADRs 004/005](adr/004-persistence.md).
- Checks: workspace fmt/Clippy/tests passed (104 including 4 compile-fail; 13 optional ignored); storage's added sixth regression passed afterward. Native/WASM all-target P02/web/remote Clippy passed; Chromium/Firefox/WebKit storage journeys passed.
- Reference/tooling: inventory and tracked evidence checks passed; 7 local reference tests passed; 71 tooling tests passed/8 optional skips; Node syntax passed. See verification for exact commands and follow-up checks.
- P02 gaps: P02.05 copied historical settings/queue imports and backup/rollback; P02.06 remember-credential policy/platform stores; P02.07 query cache/cancellation/invalidation and queue/query/diagnostic bounds. Full P02 exit not claimed.
- P01: [matrix and open gates](p01/README.md), [prepared handoff](p01/prepared-handoff.md), [playback ADR](adr/003-playback-boundaries.md). Physical renderer/input/performance, fallback/device/browser/preset and distribution evidence remain open.
- Known P01 limits: prior96a8a6d hosted failure and one MPRIS seek deadline remain unexplained; P02 did not audit or expand those prototypes.
- Review: one independent Sol medium review satisfied; no implementation findings. Stale P01 sequencing paragraph fixed and documentation checks passed.
- Next: bounded P02.05 copied historical import/rollback characterization after this delivery. Partial Navidrome vertical slice remains the intended application direction after these foundations.
- Workflow: Sol high orchestration, Terra medium implementation (usage-limited this task), authorized Sol medium completion escalation, one final independent Sol medium review. See [setup](development-workflow.md).
