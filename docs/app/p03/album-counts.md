# Bounded Subsonic album counts

P03.04 adds get_album_count using existing portable album query types and local transport. It implements the source's separate search, selected-artist, favorite and general album-list count paths. No concrete HTTP, calendar provider, backend facade, cache or new dependency is added. See [ADR-022](../adr/022-subsonic-album-counts.md) and [migration map](../migration/p03-album-counts.csv).

Search and general counts fetch 500-row pages until a shorter page arrives. Selected artists are fetched serially and counted in source order; a non-200 artist result resets the running count, an explicitly characterized source rule. Favorites count raw album rows. A general-path x-total-count can short-circuit after successful HTTP/envelope validation, including replacing a previously accumulated count. The caller provides a captured calendar year for source minimum-year defaults, separate from the monotonic deadline clock.

Only the first folder/genre is forwarded. Search takes precedence over artist selection, and a supplied empty artist selection takes precedence over favorites and returns zero without requests. Zero year bounds, ignored sort/page/filter fields and raw array counting retain source behavior. This is not a normalized album list or a snapshot-consistent library estimate.

TransportResponse now carries optional selected total-count header metadata. It stays outside domain, is fully redacted in Debug and has a 128-byte UTF-8 value cap. Body plus that value must fit the response-byte limit before parsing. Header numeric conversion uses checked nonnegative safe integers; existing HTTP/envelope failures cannot be bypassed. Concrete transports still need independent full-header/streaming reception bounds and cancellation/deadline wakeups.

Root/request/artist/observed-row work is bounded, including repeated full pages and duplicate selectors. Errors and stale responses return no partial count. Pending, between-page and final completion guards remain local/non-Send, and final cancellation/deadline checks override parsing/header outcomes.

## Verification

Pinned Rust/Cargo 1.92.0 uses the [established launcher](../p02/README.md#reproduce-checks); no lockfile/dependency change. Application/CI builds consume tracked Rust/tests/fixtures. The optional original-expression replay accepts only explicit hash-verified source/compiler inputs:

```sh
node tools/app/p03-album-count-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Forty original-controller cases cover exact endpoint parameters, branch precedence, page offsets, year defaults, headers and malformed observations. Five grouped count tests and the 11 protocol tests pass after the selected-header addition; existing transport doubles explicitly initialize the new optional field. A fixture-constructor lifetime compile issue was fixed with closures and the exposed tests reran successfully. Default tooling validates provenance and rejects source tampering before loading the compiler. Workspace tests: 277 passed, zero failures, 13 existing optional runtime tests ignored; adapter: 57 passed. Workspace/native and selected WASM all-target Clippy, formatting/diff checks passed. Full tooling: 94 passed/nine optional skips (103 total). Tracked links/evidence, two expression IDs, five migration IDs and unchanged Cargo.lock passed. Seven local reference regressions, inventory integrity and Node syntax checks also passed. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed five count tests, 11 protocol tests, all 40 source cases, two tooling guards and diff checks; hosted CI is observed afterward while work continues.

No live HTTP/server/browser/native-runtime or phase completion is claimed. Windows/macOS build/tests remain excluded; Linux/WASM and both Linux AppImage CI targets remain in scope. Remaining album/song read/count algorithms and adapter integration stay open.
