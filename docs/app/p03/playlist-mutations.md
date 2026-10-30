# Subsonic playlist mutation requests

P03.05 adds create_playlist, update_playlist, delete_playlist, add_to_playlist, remove_from_playlist and set_playlist_songs with the existing acknowledgement/uncertainty contract. See [ADR-030](../adr/030-subsonic-playlist-mutations.md) and [migration map](../migration/p03-playlist-mutations.csv). No runtime, schema, cache invalidation or UI is added.

Create returns scoped server-created ID/name and a receipt. Metadata update preserves optional comment/public, including empty strings and false. Append/direct-set preserve repeated SongIds and order. Removal accepts explicit PlaylistIndex positions, not song IDs or automatically converted snapshot entry identities, and forwards their order unchanged. Empty repeated arrays are omitted from the request as source does. One-request limits are checked before dispatch; no implicit chunking/retry is added.

Created-value normalization can fail after a valid acknowledgement; failures retain that count and callers must reconcile, since a resource may already exist. Response loss after an applied operation remains explicitly uncertain. Scoped IDs, item/pair/byte limits, correlation and pending/final context guards cover all six operations. Source permissions remain server-authoritative. Neither the receipts nor the disposable test server establish atomicity or live-server semantics.

## Reproduce verification

Use pinned Rust/Cargo 1.92.0 with the [established launcher](../p02/README.md#reproduce-checks). Default builds/tests need tracked inputs only. Optional source replay takes explicit hash-verified source/compiler paths:

```sh
node tools/app/p03-playlist-mutation-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Four grouped playlist mutation tests pass with 38 original-controller cases for exact routes/parameters, repeated/empty arrays, optional values and created projections. An owned in-memory playlist exercises all six operations, positional duplicate-song removal, response loss after creation and malformed metadata after a known acknowledgement. Tests cover pre-dispatch bounds, safe positions, typed errors, stale replies, pending and final context outcomes. The shared progress refactor was also followed by passing all five basic mutation tests and 11 protocol tests. Two provenance/tampering tooling tests pass. Combined verification on 2026-10-30 passed: 313 workspace tests, zero failures, 13 existing optional runtime tests ignored; 93 adapter tests. Workspace all-target and selected portable/WASM all-target Clippy, formatting, 110 tooling tests with nine optional skips (119 total), tracked evidence/links, inventory integrity, seven local reference regressions, Node syntax and diff whitespace passed. All six expression IDs and seven migration rows match the immutable inventory; Cargo.lock is unchanged. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed 20 focused Rust tests (playlist/basic/protocol), all 38 source cases, two tooling tests and diff checks. Hosted CI follows push while authorized implementation continues.

The separate replacePlaylist backup/remove/add workflow remains tracked for the next continuation with versioned server-scoped durable backups and interruption/recovery evidence. Direct setPlaylistSongs does not complete that workflow. Application snapshot validation/cache reconciliation and generic/extension-enabled disposable live-server conformance remain open. Linux/WASM only; Windows/macOS excluded; no phase exit is claimed.
