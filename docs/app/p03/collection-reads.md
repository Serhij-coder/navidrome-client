# Folder, role and recommendation reads

The bounded P03.03 continuation adds get_music_folders, get_artist_roles, get_random_songs and get_similar_songs through the existing injected local ProtocolTransport. No live HTTP implementation, dependency or runtime owner is added. Portable inputs/outputs and wire normalization stay on their established sides of the adapter boundary. See [ADR-018](../adr/018-subsonic-collection-reads.md) and [migration map](../migration/p03-collection-reads.csv).

Music folders retain duplicate IDs/order and attach the selected server. Roles return an unlocalized typed AllArtists choice followed by exact unique role strings sorted by UTF-16, including empty named roles. Random-song pages preserve server order/duplicates, forward only the first music folder and ignore played filtering, matching upstream. Similar-song results ignore folder selections and exclude only raw string IDs matching the request before normalization; numeric IDs can therefore normalize to the requested ID and still remain present. The fixtures characterize this source rule explicitly.

All four reads enforce scoped server identity, request/response correlation, byte and aggregate row-work budgets, local future cancellation and final context priority over normalized results/errors. Folder containers remain required; roles/random/similar optional result containers have their source empty fallbacks. Repeated rows consume work even when deduplicated or filtered. Per-array bounds also apply; reception allocation and wakeups remain responsibilities of a future production transport.

## Verification

Preflight: Rust/Cargo 1.92.0 through the [established launcher](../p02/README.md#reproduce-checks), with wasm32-unknown-unknown installed. No lockfile/dependency change. The optional reference runner accepts explicit source/compiler paths:

```sh
node tools/app/p03-collection-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Four grouped collection tests compare every normalized field and source request/path, plus scopes, strict-ID filtering, UTF-16 ordering, duplicate work limits, invalid identities/counts, pending cancellation/deadlines and final priority over malformed/success results. Source replay: 28 cases through 14 original expressions and four method/path declarations. Two default tooling tests check provenance and reject modified source before compiler loading. Workspace tests: 260 passed, zero failures, 13 existing optional runtime tests ignored; adapter: 40 passed. Workspace/native and selected WASM all-target Clippy, formatting and diff checks passed. Full tooling: 84 passed/nine optional skips (93 total). Tracked links/evidence, 14 expression IDs, six migration IDs and unchanged Cargo.lock passed. Seven local reference regressions, inventory integrity and Node syntax checks passed. The one independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed four collection tests, 28 source cases, two tooling guards and diff checks; hosted CI is observed after push while authorized implementation continues.

These are deterministic adapter fixtures, not live native/browser/server results. Linux/WASM checks exclude Windows/macOS; both Linux AppImage CI targets remain. General lists/counting/batching, real transport and all phase exits remain open.
