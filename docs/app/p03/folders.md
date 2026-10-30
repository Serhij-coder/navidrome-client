# Subsonic root and directory navigation

P03.03 adds get_folder using existing portable FolderQuery/Folder models. It covers synthetic root indexes and recursive music directories through local protocol transport. See [ADR-028](../adr/028-subsonic-folder-navigation.md) and [migration map](../migration/p03-folders.csv). No UI, filesystem scan, dependency, cache or persisted schema is added.

Exact root ID "0" calls getIndexes with the first selected folder, projects/filter artist names and stably sorts child folders. Root songs, shortcuts and unconsumed index fields are ignored as source does. Root image fields remain absent; normalized child image fields are null when missing. Other IDs call getMusicDirectory with only the ID, recursively normalize complete nested data, then filter/sort only immediate children. Song search covers title/album/artistName; descendants retain source normalized order. Repeated IDs/songs and server scope are preserved without queue/playlist identities.

One work budget covers all observed nodes and supplied song metadata before filtering. Input IDs/selectors/search, wire bytes, exact correlation and pending/final context guards apply. A mismatched returned directory ID is rejected as stale under the established detail-read policy, with the original observation retained in fixtures. Shared song sorts retain float-ms keys and deterministic local RNG; their explicit UTC ISO created-date policy remains unchanged. No error returns a partial tree.

## Reproduce verification

Use pinned Rust/Cargo 1.92.0 with the [established launcher](../p02/README.md#reproduce-checks). Application/CI tests use tracked inputs. Optional source replay verifies external source/compiler/Lodash hashes:

```sh
node tools/app/p03-folder-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js /tmp/feishin-p02-settings-lodash/package/lodash.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Four grouped Rust tests pass against 111 original-controller cases, all normalized fields/paths/parameters and RNG consumption. Both directions of all 21 song sorts are exercised on root and directory paths. Cases cover root synthetic fields, first-folder limits, ignored fields, Unicode folder ordering, immediate search, nested unsorted/unfiltered data, repeated songs, strict isDir choice, deep trees and explicit malformed/stale differences. Regressions also cover pre-filter aggregate budgets, scoped descendants, byte/errors/correlation and pending/final guards over nested errors. Two provenance/tampering tooling tests pass. Combined verification on 2026-10-30 passed: 304 workspace tests, zero failures, 13 existing optional runtime tests ignored; 84 adapter tests. Workspace all-target and selected portable/WASM all-target Clippy, formatting, 106 tooling tests with nine optional skips (115 total), tracked evidence/links, inventory integrity, seven local reference regressions, Node syntax and diff whitespace passed. All 13 expression IDs and six migration rows match the immutable inventory; Cargo.lock is unchanged. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed four folder Rust tests, all 111 source cases and two tooling tests. Hosted CI follows push while authorized implementation continues.

Linux/WASM only; Windows/macOS build/tests remain excluded. Remaining backend operations, concrete transport, live/browser/runtime conformance and all phase exits remain open.
