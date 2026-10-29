# Song detail read verification

2026-10-29, bounded P03.03 implementation in the isolated `p03-song-reads` checkout. Linux x86_64 and WASM compile/lint scope only. Integration, independent final review, task commit/push and STATUS/general documentation updates belong to the orchestrator. No phase exit is claimed.

## Target checks

Use the established Rust 1.92.0 launcher with a task-specific target directory:

```sh
export PATH=/tmp/feishin-p01-cargo/bin:$PATH
export CARGO_HOME=/tmp/feishin-p01-cargo
export RUSTUP_HOME=/tmp/feishin-p01-rustup
export CARGO_TARGET_DIR=/tmp/feishin-p03-song-reads-target
cargo test -p feishin-subsonic --locked --offline
cargo clippy -p feishin-subsonic --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-subsonic --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 tools/app/check-p00.py
node --check tools/app/p03-song-reference.cjs
```

The focused Cargo suite passed 28 tests: seven request foundation, 11 protocol and ten song-read tests. No test was ignored; crate unit/doc targets contain zero tests. Native and WASM all-target Clippy passed with warnings denied. The new top-level [song test target](../../../tests/subsonic/song_reads.rs) is explicitly wired through the [crate manifest](../../../crates/subsonic/Cargo.toml); no dependency or lockfile change was needed. Formatting, tracked documentation/link checks and replay-script syntax passed.

Coverage includes all fields emitted by the source detail normalizer, domain-only absent values, source defaults/empty arrays, repeated remixers, contributor grouping, scoped genres/song identity, same song IDs on different selected servers, malformed consumed fields and numeric bounds, numeric negative zero, request-ID/generation mismatches, HTTP/protocol/network categories, request/response/item limits, merged-artist capacity, pre/pending/post-completion cancellation and deadlines. Follow-up regressions also verify pre-dispatch nonblank/128-byte profile identity bounds without capping source song IDs, and completion-context priority over normalization/returned-ID errors using a scripted clock. Local doubles capture non-Send `Cell`/`RefCell` state and require no executor or network.

The default [song tooling guards](../../../tests/tooling/test_p03_song_reads.py) verify fixture baseline/source hashes against tracked source-audit evidence, compiler hash agreement, selected expression provenance and synthetic success/failure records. A tampered-source test requires a hash-mismatch error before the deliberately missing compiler can be opened or expressions executed. `python3 -m unittest discover -s tests/tooling -p 'test_p03*.py'` passed four tests with one existing optional full foundation replay skipped (explicit source/compiler/dependency environment not supplied); both new song guards passed. The full song replay remains the separately recorded manual check below.

## Controlled source replay

The tracked [fixture](../../../tests/fixtures/p03/subsonic-song-reads.json) contains the exact source and compiler hashes plus nine selected expression hashes. Source enum bindings are extracted from the hash-checked domain-types source. The optional local baseline/toolchain is needed only to regenerate/verify reference observations, never to compile or test the Rust application:

```sh
node tools/app/p03-song-reference.cjs /tmp/feishin-p03-song-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
```

Passed 18 synthetic cases: 15 normalized results and three characterized source TypeErrors for inherited contributor role names. `/tmp/feishin-p03-song-source` contains only the three fixture-listed source files extracted from the pinned ignored archive, with original relative paths. The tool verifies hashes before expression execution, checks function hashes on ordinary replay, and runs no upstream startup code, installers, lifecycle hooks or requests. Appending `--write` explicitly regenerates the tracked fixture after source/compiler hash validation.

The Rust comparison checks every emitted normalized field while projecting only documented representation differences: integer-millisecond truncation, absent playlist item ID as null, and missing gain/peak components as null. Integer and floating JSON numbers compare numerically; no field/value normalization other than those stated is applied. Source TypeErrors map to fixed malformed-data errors. The initial copied explicit-status replay binding was corrected to source uppercase enum values; the final replay extracts enums directly and the affected Rust/replay checks passed afterward. Negative-zero and inherited-role findings were fixed and covered before final checks. After the profile identity and completion-context follow-up, all 28 adapter tests, native/WASM Clippy, formatting, documentation checks and static migration/fixture provenance checks passed again; the unchanged JavaScript source replay was not re-executed and fixtures were not regenerated.

Applicable ignored baseline checks ran from the original local reference checkout: `python3 tools/verify-inventory.py`, `python3 -m unittest discover -s tools -p 'test_*.py'` (seven tests), and `node --check tools/inventory-upstream.cjs`. These tools remain optional local evidence, not application build/CI inputs.

## Limits

Deterministic doubles and replay are not live server conformance. No production HTTP/TLS/redirect/CORS policy, actual network cancellation, browser runtime execution, streaming allocation budget, native audio/device test, AppImage test or end-to-end client adoption is implemented or verified. The caller owns base/auth/server association. Remaining P03.03 reads and P03.04 pagination/count/batching, broader P03 work and phase exits remain open. Combined workspace checks and the one independent final review are still required at integration.

## Primary integration

The primary orchestrator copied the isolated read implementation onto `8d0c49f`, alongside the disjoint P02.07 payload codec. Existing destination baselines matched; domain, dependencies and Cargo.lock are unchanged. Final combined checks passed: 248 workspace tests including four compile-fail doctests, 13 optional runtime cases ignored; workspace fmt/all-target Clippy; domain/storage/Subsonic/web/remote all-target WASM Clippy; tracked P00/link/inventory checks; 78 tooling tests passed/nine optional skips; seven ignored-reference-tool tests; new and existing Node syntax checks. The primary reran the 18-case original-expression replay successfully. All 11 song migration IDs and four payload IDs validate; the song map was normalized to the existing CSV header convention and LF. The one final independent Sol medium reviewer found no actionable findings after inspecting actual changes and the pinned source. It independently passed four payload tests, ten song-read tests, two provenance guards and all 18 original-expression replay cases. Hosted CI remains pending. This is integration evidence, not a new runtime/server/phase claim.
