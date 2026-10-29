# Project scratchpad

- Stage: P02 foundations/storage/import journal and active durable-state repository implemented. P00/P01/P02 exits remain open; not a usable client.
- Sequencing: [P02 exception](p01/exit-decision.md) keeps P01 open; address it only for concrete implementation blockers. No release/parity gate waived.
- Scope: maintainer excludes Windows/macOS build/test runs. Current CI/verification covers Linux and WASM/browser; both Linux AppImage CPUs retained. [ADR002](adr/002-target-matrix.md).
- Domain: portable normalized models/catalogs, scoped IDs, checked units, bounded example owner and !Send contracts; no production backend/P05 reducer/P06 engine claimed.
- Import: copied inputs only; retained settings0-34 explicit early/reset/transform paths. Partial schema, no credential/raw-source storage or full historical parity.
- Stage journal: isolated current+one-backup; immutable review, stale guards, idempotence and undo/redo. Journal rollback affects staging only.
- Active repository: whole reviewed stage replaces retained active data in one new envelope with backup/revision. Missing active bootstraps legacy profile read-only; initial undo restores it. Legacy ignored after activation.
- Ownership: one mutable synchronous worker boundary; target/stage/initial-profile byte guards. No cross-process/tab CAS, cached publication or automatic uncertain-write retry. Shell/worker integration remains open.
- Checks: workspace tests160 pass incl4 compile-fail/13 optional ignored; final affected storage tests62 pass incl state9. Workspace fmt/Clippy and native/WASM all-target checks pass.
- Browser: Chromium141/Firefox142/WebKit26 each pass18 new active-state assertions, including adoption/reload/undo/redo/stale/quota/future protection.
- Tooling/reference: P00/link/inventory checks and7 local reference tests pass; tooling72 pass/8 optional skips. No source replay needed for new target adoption policy.
- Records: [adoption contract](p02/state-adoption.md), [verification](p02/state-adoption-verification.md), [migration](migration/p02-state.csv), [ADR004](adr/004-persistence.md).
- Prior delivered:c4e6505 pushed; [hosted run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40828231313) passed all former seven jobs. Historical Windows/macOS results remain retained.
- Review: one independent Sol medium reviewer satisfied after bounds fix, regression and passing affected checks; focused re-review independently passed state9 and diff checking.
- Gaps: full settings/queue metadata and real older-user exports P02.05; production shell/worker composition P02.05/P08; P02.06 credential policy/stores; P02.07 cache/cancellation.
- P01: [remaining gates](p01/README.md) remain open; no unrelated prototype expansion this task.
- Next: wire this repository through a bounded application worker before shell consumption; then credential/cache continuations.
- Workflow: Sol high orchestration, Terra medium implementation, one final independent Sol medium review. See [setup](development-workflow.md).
