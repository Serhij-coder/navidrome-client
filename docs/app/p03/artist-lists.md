# Artist and album-artist lists/counts

P03.03/P03.04 adds get_album_artists, get_artists, get_album_artist_count and get_artist_count through existing portable query/models and local transport. The Artist wrapper keeps the semantic artist category distinct while retaining source-normalized album-artist metadata. No dependency, wire DTO in domain, HTTP runtime or facade is introduced. See [ADR-021](../adr/021-subsonic-artist-lists.md) and [migration map](../migration/p03-artist-lists.csv).

Both lists flatten complete server indexes and preserve repeated IDs. Album artists optionally filter true favorites; artists optionally filter roles before normalization and ignore favorite selection. Search applies after normalization. Missing/null roles pass an active role filter, empty role arrays do not, and exact matching role arrays pass. Only the first selected folder reaches the server.

Name and album-count sorts have their source behavior, including name-ascending secondary ordering for count ties. Every other declared sort retains order: source rating is null, favorite sorting reads a missing field, and remaining variants use the unchanged default. Fixtures explicitly characterize these limitations rather than implying effective sorting/capabilities. Similar-artist metadata is retained as present arrays, with endpoint-specific zero-rating handling distinct from artist info. Malformed unconsumed album data is ignored.

Paging uses the existing source-compatible complete-array helper. Counts use the same normalized/filter/sort flow with offset zero and return full filtered totals. Aggregate work includes root, indexes, raw artist rows and consumed role/similar rows before paging/exclusion. Existing byte/scoped identity/correlation and local future guards apply, including final count/wrapper projection checks. Query text and folder selection are bounded; no runtime/UI/reception bound is claimed.

## Verification

Pinned Rust/Cargo 1.92.0 uses the [established launcher](../p02/README.md#reproduce-checks). No dependency/lockfile change. Optional source replay uses explicit hash-verified source/compiler/Lodash inputs; builds/CI use tracked inputs only:

```sh
node tools/app/p03-artist-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js /tmp/feishin-p02-settings-lodash/package/lodash.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Source replay: 83 cases through eight original expressions/controllers and one API declaration. Four grouped artist tests compare every emitted field/request dimension, all declared sort variants, filter order, budgets, scopes, invalid inputs and error/context paths. Existing album/detail/enrichment tests verify shared-helper extraction preserves their behavior. The initial fixture harness called a nonexistent enum wire method; it was corrected to the existing as_str API and the full adapter reran successfully. A Clippy nested-if finding was fixed and affected tests/checks rerun. Workspace tests: 272 passed, zero failures, 13 existing optional runtime tests ignored; adapter: 52 passed. Workspace/native and selected WASM all-target Clippy, formatting/diff checks passed. Full tooling: 92 passed/nine optional skips (101 total). Tracked links/evidence, eight expression IDs, eight migration IDs and unchanged Cargo.lock passed. Seven local reference regressions, inventory integrity and Node syntax checks passed. Existing album 18 and enrichment 26 source replays passed after shared-helper extraction. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed 12 artist/album/enrichment Rust tests, all 83 source cases, two tooling guards and diff checks; hosted CI is observed afterward while implementation continues.

This is deterministic adapter evidence, not live server/browser/native runtime or phase completion. Linux/WASM only; Windows/macOS build/tests excluded. The earlier intermittent MPRIS CI cause remains unknown; diagnostic commit 4c72c16 passed all five jobs without an engine behavior change.
