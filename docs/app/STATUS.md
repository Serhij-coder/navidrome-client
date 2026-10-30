# Project scratchpad

- Stage: bounded P02 foundations/storage/cache plus P03.01-.04 request/protocol/library reads and counts. P00/P01/P02/P03 exits remain open; not a working client.
- Sequencing: [P02 exception](p01/exit-decision.md) keeps P01 open; only address concrete blockers. Continued bounded P03 authorized; no exit waiver.
- Scope: Linux/WASM/browser checks only; Windows/macOS excluded. Both Linux AppImage CPUs retained.
- CI: album lists b4fee8e [running](https://github.com/Serhij-coder/navidrome-client/actions/runs/40991422869), portable/WASM/both AppImages passed; counts and prior cohorts all five passed. Earlier intermittent MPRIS cause unknown; assertion requested.
- Current: primary implements [song counts](p03/song-counts.md) personally; 44 source cases and 33 fixed-dataset boundary scenarios pass. Combined checks pass; independent review found no actionable findings.
- Reads: details plus ordered playlist entries/IDs and folder/role/random/similar collections. [Collections](p03/collection-reads.md), [local lists](p03/local-lists.md).
- Nested rules: repeated songs retained; duplicate disc keys last-wins; raw nullable disc lookup differs from normalized zero-to-one discNumber.
- Limits: aggregate entities/supplied metadata work budget, plus per-collection checks; no heap or streaming reception guarantee.
- Source differences: missing/null title falls back to name for sortName; checked numeric/count/disc policies and root stale-ID guard explicit in ADR016.
- Checks: song counts 287 workspace/67 adapter pass, 13 optional ignored; native/WASM Clippy, fmt; tooling 98 pass/9 skips. Independent Sol medium review: no actionable findings.
- [Album contract](p03/album-reads.md), [ADR016](adr/016-subsonic-album-reads.md), [migration](migration/p03-album-reads.csv), [verification](p03/album-reads-verification.md).
- Prior: getSong18-case source replay, guarded correlation/cancel/deadline; bounded payload codecs/encoded cache Weight, retained queue identity and durable capped writer.
- Storage/credentials: isolated profiles/import journal/active state and bounded workers; optional native preferences and Linux Secret Service. Consent/revocation/prompt/shell integration remains open.
- Gaps: remaining P03 reads/pagination/counting, production HTTP/timers, live conformance and adoption; P02 complete imports/consumers/runtime metadata gaps remain explicit.
- Playlist: positional entry IDs retain repeats; aggregate work bound and final context guards. [Contract](p03/playlist-reads.md); [migration](migration/p03-playlist-reads.csv). Independent Sol medium review: no actionable findings.
- Next: song-count checks/review/commit/push; then song lists/sorts and plan continue without stopping at task completion.
- P01 [remaining gates](p01/README.md) stay open; no unrelated prototype expansion or production reducer/engine claims.
- Workflow: sole Sol high orchestrator, direct implementation requested; final Sol medium review. [Setup](development-workflow.md).
