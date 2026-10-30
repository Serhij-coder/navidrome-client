# Combined Subsonic search

P03.03/P03.04 adds portable SearchQuery/SearchResults and adapter search over the existing local transport. It returns normalized album artists, albums and songs in one request with independent optional limits/offsets. See [ADR-025](../adr/025-subsonic-combined-search.md) and [migration map](../migration/p03-search.csv).

Missing parameters stay omitted and zero parameters remain explicit. Search text is forwarded exactly; only the first selected folder is used. Missing/null result groups become empty arrays. Results retain server order, repeated songs and full returned rows even when the server ignores requested limits. There are no local sorts, inferred totals or new cache/runtime behavior. Album nested songs and similar-artist metadata reuse established normalization policies and server scope.

One aggregate budget covers all families and nested metadata before publication. All six windows, folder count, scoped server, search length and wire bytes are checked. Pending and final cancellation/deadline guards and exact correlation remain local/non-Send. Malformed data or a stale response returns no partial families. Existing checked field/ID/time policies and explicit representation differences remain in force; ADR-025 records additional source observations without broad coercion claims.

## Reproduce verification

Use pinned Rust/Cargo 1.92.0 with the [established launcher](../p02/README.md#reproduce-checks). Default application/CI builds use tracked inputs only. Optional source replay accepts explicit hash-verified source/compiler paths:

```sh
node tools/app/p03-search-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Five grouped Rust tests pass, including 23 original-controller fixtures comparing all normalized fields and exact requests, independent/zero/omitted windows, cross-family nested bounds, scoped duplicate songs, typed failures, byte limits, stale replies and pending/final context guards. The initial rich fixture incorrectly supplied a string releaseDate; the failing test was rerun after replacing it with valid object data and retaining the string observation as a separate explicit target-error case. Two provenance/tampering tooling tests pass. Combined verification on 2026-10-30 passed: 292 workspace tests, zero failures, 13 existing optional runtime tests ignored; 72 adapter tests. Workspace all-target and selected portable/WASM all-target Clippy, formatting, 100 tooling tests with nine optional skips (109 total), tracked evidence/links, inventory integrity, seven local reference regressions, Node syntax and diff whitespace passed. All 14 expression IDs and six migration rows match the immutable inventory; Cargo.lock is unchanged. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed five search Rust tests, all 23 original cases and both search tooling tests. Hosted CI follows push while authorized implementation continues.

Linux/WASM checks only; Windows/macOS excluded. No production HTTP, browser runtime, live-server, packaged-adapter or phase parity is established. Remaining song-list/folder/top/favorite algorithms and adapter integration stay open.
