# Project scratchpad

- Stage: bounded P02 foundations/storage/cache plus P03.01-.03 request/protocol/detail reads. P00/P01/P02/P03 exits remain open; not a working client.
- Sequencing: [P02 exception](p01/exit-decision.md) keeps P01 open; only address concrete blockers. Continued bounded P03 authorized; no exit waiver.
- Scope: Linux/WASM/browser checks only; Windows/macOS excluded. Both Linux AppImage CPUs retained.
- Latest delivery341df20: [all5 CI jobs passed](https://github.com/Serhij-coder/navidrome-client/actions/runs/40890040081), including both AppImages.
- Current task: primary implements P03.03 album and album-artist detail personally; no second orchestrator per maintainer correction. Required final reviewer remains independent.
- Reads: getAlbum/getArtist reuse guarded local transport and song helpers; optional albums preserved through new portable AlbumArtistDetail wrapper.
- Nested rules: repeated songs retained; duplicate disc keys last-wins; raw nullable disc lookup differs from normalized zero-to-one discNumber.
- Limits: aggregate entities/supplied metadata work budget, plus per-collection checks; no heap or streaming reception guarantee.
- Source differences: missing/null title falls back to name for sortName; checked numeric/count/disc policies and root stale-ID guard explicit in ADR016.
- Checks: workspace252 pass incl4 compile-fail/13 optional ignored; adapter32 pass; native/workspace and selected WASM Clippy pass; source18-case replay pass.
- [Album contract](p03/album-reads.md), [ADR016](adr/016-subsonic-album-reads.md), [migration](migration/p03-album-reads.csv), [verification](p03/album-reads-verification.md).
- Prior: getSong18-case source replay, guarded correlation/cancel/deadline; bounded payload codecs/encoded cache Weight, retained queue identity and durable capped writer.
- Storage/credentials: isolated profiles/import journal/active state and bounded workers; optional native preferences and Linux Secret Service. Consent/revocation/prompt/shell integration remains open.
- Gaps: remaining P03 reads/pagination/counting, production HTTP/timers, live conformance and adoption; P02 complete imports/consumers/runtime metadata gaps remain explicit.
- Review: one independent Sol medium reviewer found no actionable findings; four album tests,18 source cases and two tooling guards independently pass.
- Next: await hosted CI, then continue bounded P03 playlist/detail reads; remaining phase gates stay open.
- P01 [remaining gates](p01/README.md) stay open; no unrelated prototype expansion or production reducer/engine claims.
- Workflow: sole Sol high orchestrator, direct implementation requested; final Sol medium review. [Setup](development-workflow.md).
