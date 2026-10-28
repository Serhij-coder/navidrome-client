# Project scratchpad

- Stage: P02.01-P02.03 foundations, bounded P02.04 persistence and bounded P02.05 copied-import preview/journal implemented. Reviewer P2 recovery fix tested and focused rereview satisfied; P00/P01/P02 exits remain open. Not a usable music client.
- Sequencing: [P02 exception and authorized continuation](p01/exit-decision.md) keep P01 open; address P01 only for concrete implementation blockers. No release/parity evidence waived.
- Domain: portable normalized models/catalogs, distinct scoped IDs, checked times, bounded example owner and !Send contracts. No P03/P05/P06 production adapters/reducer/engine claimed.
- Persistence: isolated schema-v1 profile store and separate import journal; native atomic files/browser single keys. Not adopted by shells; synchronous parsing/native I/O need production worker composition.
- Import: explicit copy bytes only; settings14/34 partial, auth2, player3/4, timestamp1/raw. Whole column arrays, Auto DJ, duplicate song occurrences, orphan cleanup and checked shuffled/current timestamp mapping have regressions; credentials/raw unknown payloads dropped with typed reports.
- Journal: immutable reviewed candidate, current plus one backup atomically, exact-baseline stale guards, idempotence and rollback/redo. Whole-file corruption and external concurrent writers are not certified recovery cases.
- Evidence: [import matrix](p02/imports.md), [journal](p02/import-journal.md), [checks](p02/import-verification.md), [migration](migration/p02-imports.csv), [ADR-004](adr/004-persistence.md); prior [foundation](p02/README.md) retained.
- Checks: workspace fmt/Clippy/tests pass:130 including4 compile-fail,13 optional ignored. After review fix, storage imports17/journal8/profiles6 pass; native/WASM/Windows-MSVC Clippy pass; Windows-GNU/Wine31 pass.
- Browser: Chromium/Firefox/WebKit each pass profile10 and import-journal12 checks on final built fixture; synthetic fault injection separate from real storage/reload execution.
- Reference/tooling: inventory passed;7 local reference tests passed; tooling71 passed/8 optional skips; Node syntax passed. See verification for exact commands.
- Foundation delivered: dea4b4f + Windows correction9be5b00 pushed to origin/main; [9be5b00 hosted run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40759859724) passed all seven jobs.
- P02 gaps: unsupported historical versions/transforms, full settings/queue metadata and production adoption remain P02.05/P08; P02.06 remember policy/platform stores and P02.07 cache/invalidation/cancellation remain open. No full P02.05/phase exit claimed.
- P01: [open gates](p01/README.md) still require physical renderer/input/performance, fallback/device/browser/preset and distribution evidence. This task did not audit or expand prototypes.
- Review: one independent Sol medium review and focused rereview satisfied; wrong-current recovery fixed and affected checks rerun.
- Next: extend P02.05 historical-version coverage and adoption boundaries after this bounded delivery. Credential remembering and cache lifecycle remain separate.
- Workflow: Sol high orchestration; Terra requested authorized Sol medium escalation for broad import validation; one final independent Sol medium reviewer. See [setup](development-workflow.md).
