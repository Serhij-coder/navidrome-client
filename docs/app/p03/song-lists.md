# Subsonic song lists and local sorting

P03.03/P03.04 adds get_songs over the existing portable query/models and local protocol transport. It covers search, genre, favorites, album/artist fan-out and general-library paths with source-specific ordering, offsets and totals. See [ADR-026](../adr/026-subsonic-song-lists.md) and [migration map](../migration/p03-song-lists.csv). No dependency, concrete HTTP, UI, cache or persisted schema is added.

Search returns its requested start index; genre/general return zero while forwarding the requested wire offset. All preserve unknown totals, server order and all received rows without local sorting. Supplied empty genre IDs still choose the genre endpoint. Favorites normalize all rows, filter nonempty artist selections against album artists, sort locally and then paginate with the full filtered total. Limit zero means all remaining rows. Album-artist selection takes precedence even when empty. This differs from the separately preserved raw favorite count.

Selected album/artist requests run serially, preserve repeated selectors and songs, skip only HTTP failures and return all fetched songs without sorting/slicing. Album disc-title maps are intentionally not used on this source path. Work is bounded across requests and all observed/supplied rows, before filtering or slicing. Scoped IDs, explicit units, checked page values, byte limits, exact correlation and local cancellation/deadline guards apply; failures return no partial page.

Every declared local sort has fixtures in both directions, including source ineffective/secondary-key behavior, UTF-16 order, case-sensitive sortName, float duration ordering, ID reversal and deterministic forward shuffle. Shuffle consumes one non-Send-compatible RNG draw per row only on the local favorite path. Recently-added sorting uses an explicit UTC ISO-date policy and rejects invalid/non-ISO timestamps on that sort path; the original host-timezone and coercion observations remain in fixtures. ADR-026 records these compatibility differences without claiming general Date.parse or runtime parity.

## Reproduce verification

Use pinned Rust/Cargo 1.92.0 with the [established launcher](../p02/README.md#reproduce-checks). Default tests and CI use tracked inputs. The optional original-expression replay verifies source/compiler/Lodash hashes before execution and scopes/restores its process timezone:

```sh
node tools/app/p03-song-list-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js /tmp/feishin-p02-settings-lodash/package/lodash.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Five grouped Rust tests pass against 254 original-controller cases plus a second-zone probe, exact endpoint parameters, all normalized fields and RNG draw counts. All 21 sorts in both directions are covered across five routing paths; favorite cases compare full sorted output and local pagination is separate. Tests also cover full pre-filter work, aggregate fan-out, duplicate identity, ignored disc maps, typed errors/byte limits/staleness and pending/final/no-I/O guards. The date parser's time-component allocation was reduced to bounded iterator reads; the excessive-component regression and all song-list tests reran successfully. Two provenance/tampering tooling tests pass. Combined verification on 2026-10-30 passed: 297 workspace tests, zero failures, 13 existing optional runtime tests ignored; 77 adapter tests. Workspace all-target and selected portable/WASM all-target Clippy, formatting, 102 tooling tests with nine optional skips (111 total), tracked evidence/links, inventory integrity, seven local reference regressions, Node syntax and diff whitespace passed. All 13 expression IDs and nine migration rows match the immutable inventory; Cargo.lock is unchanged. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed five song-list Rust tests, all 254 original cases, two tooling tests and diff checks. Hosted CI follows push while authorized implementation continues.

Linux/WASM only; Windows/macOS build/tests remain excluded. Source date compatibility limits, production transport/runtime composition, folder/top/favorite/service reads, live conformance and phase exits remain open.
