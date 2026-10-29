# Local playlist and genre lists

P03.03/P03.04 now expose get_playlists, get_playlist_count and get_genres through the existing local transport. Playlist/genre list queries use the existing portable types; no new backend DTO, dependency or runtime is introduced. See [ADR-019](../adr/019-subsonic-local-lists.md) and [migration map](../migration/p03-local-lists.csv).

Both list endpoints fetch complete collections, apply source search/sort rules, normalize all retained rows and then paginate locally. Every declared sort option is covered, including stable ties, case conversion, UTF-16 string order and the source distinction between missing and null sort keys. Limit zero means all remaining rows, and a page beyond the end retains its offset and complete filtered count. Malformed retained rows fail even outside the requested page; filtering can exclude invalid metadata before normalization.

Playlist count reads have their own source behavior: they count filtered raw rows and do not normalize metadata. Without search, even malformed/null rows count toward array length. Playlist smart filtering and genre folder selections have no effect for this backend. Requests send none of these local page/search/sort/filter parameters to the server.

Root-plus-raw-row work limits apply before filtering/paging. Scoped server identity, request/response bytes, exact correlation, local non-Send futures and final context priority are tested. Paging counts are safe integers and search input has an 8192-byte UTF-8 cap. Caller still binds server/base/auth; production transport reception/timers and runtime/UI scheduling remain open.

## Verification

Pinned Rust/Cargo 1.92.0 uses the [established launcher](../p02/README.md#reproduce-checks). No dependency/lockfile change. Application builds/CI consume tracked inputs only. Optional reference replay also requires the explicitly supplied hash-verified Lodash 4.17.21 module:

```sh
node tools/app/p03-list-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js /tmp/feishin-p02-settings-lodash/package/lodash.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

The 57 source cases replay six original expressions/controllers plus two method/path declarations. Four grouped Rust tests compare every emitted normalized field and query behavior, full-work bounds, list/count agreement, errors, stale correlation, pending/final cancellation and deadlines. Inspection caught a null-key ordering mistake; the corrected comparator passes ascending/descending missing/null regression fixtures. Two default tooling guards verify provenance and pre-compiler tampering rejection. Workspace tests: 264 passed, zero failures, 13 existing optional runtime tests ignored; adapter: 44 passed. Workspace/native and selected WASM all-target Clippy, formatting/diff checks passed. Tooling: 86 passed/nine optional skips (95 total). Tracked evidence/links, six expression IDs, six migration IDs and unchanged Cargo.lock passed. Seven local reference regressions, inventory integrity and Node syntax checks also passed. The one independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed four list tests, all 57 source cases and two provenance/tampering guards; hosted CI is observed after push while implementation continues.

Deterministic adapter evidence does not certify live HTTP/native/browser/server results. General P03.04 probing/count batching, other reads and all phase exits remain open. Linux/WASM only; Windows/macOS build/tests excluded.
