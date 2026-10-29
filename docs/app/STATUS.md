# Project scratchpad

- Stage: bounded P02 foundations/storage/cache plus P03.01-.03 request/protocol/detail reads. P00/P01/P02/P03 exits remain open; not a working client.
- Sequencing: [P02 exception](p01/exit-decision.md) keeps P01 open; only address concrete blockers. Continued bounded P03 authorized; no exit waiver.
- Scope: Linux/WASM/browser checks only; Windows/macOS excluded. Both Linux AppImage CPUs retained.
- Latest delivery dedf7e8: [all five CI jobs passed](https://github.com/Serhij-coder/navidrome-client/actions/runs/40949805465), including both Linux AppImages.
- Current task: primary implements P03.03 playlist metadata/ordered entries/ID projection personally; required final independent review remains separate.
- Reads: getAlbum/getArtist reuse guarded local transport and song helpers; optional albums preserved through new portable AlbumArtistDetail wrapper.
- Nested rules: repeated songs retained; duplicate disc keys last-wins; raw nullable disc lookup differs from normalized zero-to-one discNumber.
- Limits: aggregate entities/supplied metadata work budget, plus per-collection checks; no heap or streaming reception guarantee.
- Source differences: missing/null title falls back to name for sortName; checked numeric/count/disc policies and root stale-ID guard explicit in ADR016.
- Checks: workspace256 pass/13 optional ignored; adapter36 pass; workspace and selected WASM Clippy pass; playlist25/album18 source cases pass.
- [Album contract](p03/album-reads.md), [ADR016](adr/016-subsonic-album-reads.md), [migration](migration/p03-album-reads.csv), [verification](p03/album-reads-verification.md).
- Prior: getSong18-case source replay, guarded correlation/cancel/deadline; bounded payload codecs/encoded cache Weight, retained queue identity and durable capped writer.
- Storage/credentials: isolated profiles/import journal/active state and bounded workers; optional native preferences and Linux Secret Service. Consent/revocation/prompt/shell integration remains open.
- Gaps: remaining P03 reads/pagination/counting, production HTTP/timers, live conformance and adoption; P02 complete imports/consumers/runtime metadata gaps remain explicit.
- Playlist: positional entry IDs retain repeats; aggregate work bound and final context guards. [Contract](p03/playlist-reads.md); [migration](migration/p03-playlist-reads.csv). Independent Sol medium review: no actionable findings.
- Next: push reviewed playlist increment and observe CI while continuing P03 collection reads; do not stop at task completion, per maintainer instruction.
- P01 [remaining gates](p01/README.md) stay open; no unrelated prototype expansion or production reducer/engine claims.
- Workflow: sole Sol high orchestrator, direct implementation requested; final Sol medium review. [Setup](development-workflow.md).
