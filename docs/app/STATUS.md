# Project scratchpad

- Stage: P02 foundations/storage/import journal implemented; retained settings versions0-34 now handled through explicit early/reset/transform paths. Checks and independent review passed. P00/P01/P02 exits remain open; not a usable client.
- Sequencing: [P02 exception/continuation](p01/exit-decision.md) keeps P01 open; address it only for concrete implementation blockers. No release/parity gate waived.
- Domain: portable normalized models/catalogs, scoped IDs, checked units, bounded example owner and !Send contracts; no production backend/P05 reducer/P06 engine claimed.
- Import: copies only, immutable partial preview. Settings0-9 return early,10/11 reset retained settings,12-34 transform. Profiles/selected server and queue/current/position survive resets; malformed reset envelopes fail without writes.
- Reset precedence: authoritative reset dominates weaker copies; selected supplemental reset affects its category only. Source null-reset permissiveness is deliberately rejected at the typed boundary; full UI default hydration remains open.
- Journal: isolated current+one-backup value, exact reviewed apply, stale guards, idempotence and undo/redo. No source/profile activation; worker composition and external concurrent writers remain open.
- Checks: workspace fmt/Clippy/tests pass153 incl4 compile-fail;13 optional ignored. Storage53 pass; native/WASM/Windows-MSVC Clippy pass; Windows-GNU/Wine53 pass.
- Source proof: original273 column/110 AutoDJ replays unchanged; new56 early/reset cases and5 malformed observations matched. Fixtures are synthetic/source-derived, not older real-user exports.
- Browser: Chromium/Firefox/WebKit each pass profile10 + import/reset17 on final WASM fixture, including reset save/reload/undo; quota/security fault injection remains explicit.
- Reference/tooling: inventory/P00 checks pass;7 local reference tests pass; tooling72 pass/8 optional skips; Node syntax and both replay-mode tamper guards pass.
- Records: [early/reset scope](p02/early-settings.md), [verification](p02/early-settings-verification.md), [migration](migration/p02-settings-transforms.csv), [imports](p02/imports.md), [ADRs004](adr/004-persistence.md)/[009](adr/009-source-differences.md).
- Prior delivered:0121a86 pushed; [hosted run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40791476724) passed all seven jobs. Older foundation/import increments remain delivered.
- Gaps: full settings/queue metadata and production state adoption remain P02.05/P08; P02.06 credential policy/platform stores and P02.07 cache/cancellation remain open. Numeric version handling is not full historical parity.
- P01: [remaining gates](p01/README.md) still need physical renderer/input/performance, fallback/device/browser/preset/distribution evidence; no prototype expansion this task.
- Review: one independent Sol medium review satisfied; stale ADR coverage corrected and documentation checks rerun.
- Next: implement the staged-data adoption boundary before credential/cache continuations.
- Workflow: Sol high orchestration, Terra medium helper implementation, one final independent Sol medium review. See [setup](development-workflow.md).
