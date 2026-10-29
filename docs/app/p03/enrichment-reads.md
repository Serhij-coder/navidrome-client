# Artist enrichment and album/artist radio reads

P03.03 adds get_album_artist_info, get_album_radio and get_artist_radio using existing portable models and local transport. These are backend reads without a playback owner, radio station integration, UI, dependency or real HTTP implementation. See [ADR-020](../adr/020-subsonic-enrichment-reads.md) and [migration map](../migration/p03-enrichment-reads.csv).

Album radio uses getSimilarSongs and artist radio uses getSimilarSongs2. Both preserve complete ordered recommendations and repeated songs, including matching source IDs. Artist info preserves explicit optional biography/similar-artist shapes, nullish cover fallback, empty covers and zero ratings. Only non-200 HTTP info results become None for source compatibility; protocol/network/validation/context failures remain errors. Missing/null artist IDs and malformed required metadata are deliberately rejected; numeric cover IDs become safe strings.

Server/entity identity checks, exact correlation, request/response limits, aggregate work bounds and final context priority cover all operations. The info HTTP fallback also rechecks cancellation/deadline. Futures remain local/non-Send. Caller binds base/auth/server and must retain server/artist context for related enrichment values. Actual transport reception, wakeups and runtime scheduling remain open.

## Verification

Use pinned Rust/Cargo 1.92.0 and the [established launcher](../p02/README.md#reproduce-checks); no lockfile or dependency change. Application builds/CI use tracked inputs. Optional original-expression replay requires explicit hash-verified source/compiler paths:

```sh
node tools/app/p03-enrichment-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Source replay: 26 cases through 12 original expressions/controllers and three API route declarations. Four grouped Rust tests compare normalized fields/parameters/paths and exercise scopes, duplicates, optional shapes, zero ratings, limits, fixed errors, pending and final context guards. A test-only integer-versus-float JSON-zero assertion was corrected and the exposed test plus all four affected groups reran successfully; adapter behavior was unchanged. Two default tooling guards check provenance and reject modified source before compiler loading. Workspace tests: 268 passed, zero failures, 13 existing optional runtime tests ignored; adapter: 48 passed. Workspace/native and selected WASM all-target Clippy, formatting/diff, tracked links/evidence, 12 expression IDs, four migration IDs and unchanged Cargo.lock passed. Current combined tooling: 90 passed/nine optional skips (99 total), including the separate CI diagnostic increment. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed four enrichment Rust tests, two tooling guards, all 26 source cases and diff checks; hosted CI is observed afterward while implementation continues.

No live HTTP/server/browser result or phase completion is claimed. Current checks cover Linux/WASM; Windows/macOS build/tests are excluded, both Linux AppImage CI jobs remain required.
