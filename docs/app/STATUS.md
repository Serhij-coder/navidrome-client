# Project scratchpad

- Stage: P02.01-P02.03 foundations, bounded P02.04 storage and P02.05 import/journal implemented; retained historical settings14-34 transforms verified and independent review satisfied. P00/P01/P02 exits remain open; not a usable client.
- Sequencing: [P02 exception and continuation](p01/exit-decision.md) keep P01 open; address P01 only for concrete implementation blockers. No release/parity gate waived.
- Domain: portable normalized models/catalogs, scoped distinct IDs, checked units, bounded example owner and !Send contracts. No production backend/P05 reducer/P06 engine claimed.
- Storage/import: isolated profile store plus separate single-value current/backup import journal; immutable preview/apply and stale-guarded rollback. Source copies/secrets are not saved; shell/owner adoption and worker composition remain open.
- History: settings14-34 retained columns and supplied AutoDJ normalize by selected category version. Whole arrays replace after migration; absent categories preserve baseline with legacy-default warning. Unsupported fields/transforms and versions0-13 remain explicit.
- Source evidence: exact migration/merge helper replay matched273 column and110 AutoDJ cases; legacy-key and duplicate-insertion observations recorded. Target keeps existing column ID/configuration for uniqueness/idempotence, documented in ADR004/009.
- Checks: workspace fmt/Clippy/tests pass144 incl4 compile-fail;13 optional ignored. Storage44 integration pass; native/WASM/Windows-MSVC Clippy pass; Windows-GNU/Wine44 pass.
- Browser: Chromium/Firefox/WebKit each pass profile10 + import/history13 on final fixture; normalized preview/apply/idempotence/rollback executed in real storage. Synthetic quota/security injection remains separate.
- Reference/tooling: inventory and P00 evidence checks passed;7 local reference tests pass; tooling72 pass/8 optional skips; Node syntax and source-replay tamper guard pass.
- Records: [history scope](p02/settings-history.md), [verification](p02/settings-history-verification.md), [migration](migration/p02-settings-transforms.csv), [import guide](p02/imports.md), [ADR-004](adr/004-persistence.md), [ADR-009](adr/009-source-differences.md).
- Delivered imports:7047763 pushed to origin/main; [hosted run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40767368341) passed all seven jobs. Foundation/Windows fixes remain delivered.
- Gaps: settings0-13 reset/early-return semantics, full settings/queue metadata and production adoption remain P02.05/P08; P02.06 credential policy/platform stores and P02.07 cache/cancellation remain open. No phase/full historical parity claimed.
- P01: [open gates](p01/README.md) still need physical renderer/input/performance, fallback/device/browser/preset/distribution evidence. No prototype expansion this task.
- Review: one independent Sol medium review satisfied; stale map gap fixed and documentation checks passed.
- Next: address remaining P02.05 older-version/reset cases and adoption boundary before credential/cache continuations.
- Workflow: Sol high orchestration, Terra medium helper implementation and one final independent Sol medium reviewer. See [setup](development-workflow.md).
