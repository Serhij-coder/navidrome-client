# Subsonic composed song views and service metadata

P03.03 adds get_favorite_songs, get_top_songs, get_internet_radio_stations and get_scan_status. Typed query modes and scoped artist identity reuse existing portable models and bounded local transport. See [ADR-027](../adr/027-subsonic-catalog-reads.md) and [migration map](../migration/p03-catalog-reads.csv). No dependency, runtime, schema, UI, radio player or scan mutation is added.

Favorite/rating and personal-top reads compose the tested selected-artist song path, apply source thresholds and stable multi-key ordering, preserve duplicates, ignore limit and retain the unfiltered underlying total. Nullable rating order remains explicit. Community top songs gates ID-versus-name requests by nonempty advertised feature versions; optional count including zero does not cause local truncation. All source compositions consume no RNG.

Radio metadata preserves station order, duplicate string IDs, source image/homepage rules and inert URL text, adding explicit server scope. Consuming playback/navigation services still validate URLs before use. Scan metadata requires checked integer counts and a boolean flag, with optional raw last-scan text. Malformed shapes, typed protocol errors and stale replies return no partial values. Shared item/byte limits and local pending/final cancellation/deadline checks apply.

## Reproduce verification

Use pinned Rust/Cargo 1.92.0 with the [established launcher](../p02/README.md#reproduce-checks). Default builds/tests use tracked inputs; optional source replay accepts explicit hash-verified source/compiler/Lodash paths:

```sh
node tools/app/p03-catalog-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js /tmp/feishin-p02-settings-lodash/package/lodash.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Three grouped Rust tests pass: 50 original-controller cases with all normalized fields, exact request sequences and zero RNG draws; scoped/work/byte/error checks; and pending/final guards for direct and composed reads. The guard regression initially exposed malformed inner errors bypassing the final wrapper check; both composed wrappers now recheck all outcomes and the exposing test plus the complete catalog target pass after the fix. Two provenance/tampering tooling tests pass. Combined verification on 2026-10-30 passed: 300 workspace tests, zero failures, 13 existing optional runtime tests ignored; 80 adapter tests. Workspace all-target and selected portable/WASM all-target Clippy, formatting, 104 tooling tests with nine optional skips (113 total), tracked evidence/links, inventory integrity, seven local reference regressions, Node syntax and diff whitespace passed. All 19 expression IDs and seven migration rows match the immutable inventory; Cargo.lock is unchanged. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed three catalog Rust tests, all 50 source cases, two tooling tests and diff checks, and inspected combined check logs. Hosted CI follows push while authorized implementation continues.

Linux/WASM only; Windows/macOS build/tests remain excluded. Folder navigation, remaining backend operations, concrete transport, live/browser/runtime conformance and all phase exits stay open.
