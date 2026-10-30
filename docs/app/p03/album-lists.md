# Subsonic album lists and local ordering

P03.03/P03.04 adds get_albums for all source getAlbumList branches using existing portable queries/models and an explicit AlbumReadEnvironment calendar/RNG input. No concrete HTTP, UI, cache, persisted schema or new dependency is added. See [ADR-023](../adr/023-subsonic-album-lists.md) and [migration map](../migration/p03-album-lists.csv).

Search/general list paths preserve server order, requested offsets and unknown totals. They do not truncate a server response to the requested size or infer totals from headers. Supplied artist selections preserve repeated albums, serialize requests, skip HTTP-only failed artist results and return all locally sorted rows with offset 0 and observed total. True favorites normalize/sort every returned album before local pagination; limit 0 means all remaining rows. First-folder/genre and year precedence follow original endpoint-specific rules.

Local sorts preserve stable name ties, missing/null counts, source fractional-duration order, case-sensitive sort names, ID reversal and source ineffective sort branches. Deterministic forward shuffle consumes one 53-bit-compatible sample per row from the caller's Random. The same provider can be non-Send; server random/search paths do not consume it. Nested songs retain repetitions and server identity without introducing queue or playlist-entry IDs.

Two date rules are deliberate compatibility decisions. Normalized legacy dates use UTC rather than the host timezone; the fixture records a UTC/Europe-Bratislava order difference. Invalid-date ties stay stable because the source comparator can reverse them descending. Original observations remain in the fixture, and the one target tie-order correction is explicit. Normalized integer duration/name fallback projections retain existing ADR policies.

Aggregate work includes operation/artist/album roots and nested supplied rows before page slicing. Existing identity/query/byte/correlation and local cancellation/deadline guards apply, including no-I/O empty selections and final malformed/success paths. Concrete reception, wakeups, runtime scheduling and authenticated profile binding remain caller/transport responsibilities. This does not establish live endpoint, browser runtime, packaged adapter or phase parity.

## Verification

Use pinned Rust/Cargo 1.92.0 and the [established launcher](../p02/README.md#reproduce-checks). Default builds/tests use tracked inputs without ignored source or reference packages. Optional replay checks exact source/compiler/Lodash hashes and temporarily sets only its own process timezone for deterministic source observations, restoring it afterward:

```sh
node tools/app/p03-album-list-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js /tmp/feishin-p02-settings-lodash/package/lodash.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Five grouped album-list tests compare 145 original-controller cases, exact requests/paths/RNG consumption, every normalized field and targeted bounds/context regressions. Default tooling verifies provenance, exhaustive sort coverage, date differences and source tampering rejection. The source selector was tightened to arrow functions to avoid capturing a same-named wire-schema declaration; reference replay then passed. The test harness uses the existing 256 recursion limit for shared model JSON projections. Verification on 2026-10-30 passed: workspace tests 282 passed / 13 existing optional runtime tests ignored; adapter tests 62 passed; tooling 96 passed / 9 optional reference checks skipped. Workspace and selected portable/WASM all-target Clippy, formatting, 145-case original replay plus timezone probe, the existing 18-case album replay, inventory integrity, reference-tool tests and evidence/link checks passed. After the private duration sort field was renamed to duration_ms, all 62 adapter tests and native/WASM adapter Clippy passed again. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed five album-list tests, the 145-case source replay and timezone probe, two tooling tests and diff checks. Hosted CI follows commit/push while authorized implementation continues.

Linux/WASM only; Windows/macOS build/tests remain excluded. Remaining song/folder/search algorithms, adapters/runtime composition and phase exits remain open.
