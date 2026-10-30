# Project scratchpad

- Stage: bounded P02 foundations/storage/cache and P03.01-.05 library/protocol/mutation contracts. P00/P01/P02/P03 exits remain open; not a working client.
- Sequencing: [P02 exception](p01/exit-decision.md) keeps P01 open; continued bounded P03/plan progression authorized, no evidence waiver.
- Scope: Linux/WASM/browser checks only; Windows/macOS excluded. Both Linux AppImage CPUs retained.
- Current: primary implements [playlist mutation requests](p03/playlist-mutations.md) personally; 38 source cases/four grouped tests pass, plus shared progress regressions. Combined checks pass; independent review found no actionable findings.
- CI: basic mutations 26131ee [all five passed](https://github.com/Serhij-coder/navidrome-client/actions/runs/41035032160); earlier read/count cohorts passed. Earlier intermittent MPRIS cause remains unknown; assertion requested.
- Checks: playlist mutations 313 workspace/93 adapter passed, 13 optional ignored; native/WASM Clippy/fmt; tooling 110 passed/9 skips. Independent Sol medium review: no actionable findings.
- Reads: [song lists](p03/song-lists.md), [search](p03/search.md), [catalog](p03/catalog-reads.md), [folders](p03/folders.md) retain source order/repeats with bounded work and explicit date/shape policies.
- Mutation policy: typed request acknowledgements and unknown outcomes; no automatic retry/rollback or per-ID atomic claim. Source rating failure swallowing corrected; application invalidation/reconciliation remains open.
- P03 gaps: remaining playlist/radio/share/queue mutations, stream/image/download/lyrics/reporting/jukebox, production HTTP/timers, disposable live conformance and shell adoption.
- P02 gaps: full imports/settings/consumers and runtime metadata remain explicit; credential consent/revocation/prompt/shell integration stays open. [P02](p02/README.md).
- P01 [remaining gates](p01/README.md) remain open; no unrelated prototype expansion, production reducer/engine or release claims.
- Next: playlist mutation checks/review/commit/push; then versioned server-scoped replacement backups and remaining P03 contracts.
- Workflow: sole Sol high orchestrator, direct implementation requested; one final Sol medium review per coherent task. [Setup](development-workflow.md).
