# Subsonic song counts

P03.04 adds get_song_count using the existing SongListQuery and local protocol transport. Search pages, genre/general stride probes, raw favorites and album/artist fan-out retain separate source rules. See [ADR-024](../adr/024-subsonic-song-counts.md) and [migration map](../migration/p03-song-counts.csv). This adds no dependency, concrete HTTP, UI, cache or persisted schema.

Search pages by 500. Genre/general counts probe one song every 5000 offsets, backtrack one stride and enumerate 500-row pages to the end. Supplied empty genre IDs still select the genre endpoint; first folder/genre limits remain explicit. Favorite counts ignore artist filters even though the source favorite list applies them. Album-artist IDs take precedence over artist IDs; empty supplied album/artist selections return zero without requests. Direct and discovered albums preserve repetition and HTTP-only skip behavior.

All operations bound aggregate request/observed-row work and validate selector cardinalities. They use checked offsets, exact correlation, response/request byte caps and cancellation/deadline checks, including pending, between-request and final completion paths. Errors publish no partial count. Serial fan-out replaces source Promise.all; missing/null/scalar required album song arrays are rejected rather than counted as JavaScript flatMap artifacts. Source results remain intact in fixtures with explicit target-error annotations. A changing library can produce a stale count; no snapshot consistency is claimed.

## Reproduce verification

Pinned Rust/Cargo 1.92.0 uses the [established launcher](../p02/README.md#reproduce-checks). Default tests/CI need tracked inputs only; optional replay verifies hashes before loading the source compiler:

```sh
node tools/app/p03-song-count-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Five grouped Rust tests pass: 44 original-controller cases with exact request paths/parameters, 33 fixed-dataset boundary scenarios, bounded growing libraries/fan-out, typed errors/byte limits/staleness, and pending/between-request/final/no-I/O guards. Two tooling tests pass for provenance/characterizations and source tampering rejection before compiler loading. Combined verification on 2026-10-30 passed: 287 workspace tests, zero failures, 13 existing optional runtime tests ignored; 67 adapter tests. Workspace all-target and selected portable/WASM all-target Clippy, formatting, 98 tooling tests with nine optional skips (107 total), tracked evidence/links, inventory integrity, seven local reference regressions, Node syntax and diff whitespace passed. Both expression IDs and all six migration rows match the immutable inventory; Cargo.lock is unchanged. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed five song-count Rust tests, all 44 original cases, two tooling tests and diff checks, and inspected the combined check logs. Hosted CI follows push while authorized implementation continues.

Linux/WASM checks only; Windows/macOS excluded. Song lists/local sorts, remaining adapter operations, runtime transport and phase exits remain open. The prior [album-list commit b4fee8e](https://github.com/Serhij-coder/navidrome-client/commit/b4fee8e) is tracked separately from this increment's checks.
