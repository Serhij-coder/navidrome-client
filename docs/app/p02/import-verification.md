# Bounded P02.05 import and rollback verification

Date: 2026-10-28. Baseline: Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8`. The supported subset is documented in the [import matrix](imports.md) and [journal contract](import-journal.md); this is not a full P02.05 or phase exit. The preceding foundation revision `9be5b00` [completed all seven hosted jobs successfully](https://github.com/Serhij-coder/navidrome-client/actions/runs/40759859724).

## Implementation evidence

Tracked current-version export input is the original captured v34 fixture. Historical v14/auth2/player3/4/timestamp shapes are synthetic/source-derived, including the previously characterized v3 migration trace. The pinned source was inspected directly and stable function IDs checked against the immutable local inventory. No upstream installer, lifecycle hook or reference script was executed for this increment. No older real-user profile/export corpus was available, and compatibility outside the explicit matrix is unverified.

The importer projects supported non-secret values into a private preview and reports all omitted categories/unsupported migrations. It preserves column arrays as whole arrays, Auto DJ fields, occurrence identity and orphan cleanup. Inspection caught and fixed an initial current-selection error: album mode and track mode with an empty shuffle permutation use default order in the source. Regression tests now cover those cases separately from active valid/invalid permutations. Duration values are normalized milliseconds, while saved timestamp values are seconds; separate checked conversions prevent multiplying song duration by 1000 again.

The repository commits the exact reviewed candidate with one previous target snapshot in a single atomic bounded value. It rejects stale plans, corrupt/future schemas, invalid backups and revision exhaustion. Fixtures cover failed replacement, a durability error after a visible write, rollback/redo/reopen, interrupted temporary data, namespace isolation and current-plus-backup capacity. These checks do not certify arbitrary filesystem power loss or concurrent external writers.

## Exact local checks

Used Rust/Cargo 1.92.0 with the established [P02 launcher](README.md#reproduce-checks). The only lockfile change adds the existing dependency-free domain workspace crate as a storage dependency for typed identity/time bridges; no external package version changed.

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed.
- `cargo test --workspace --locked --offline`: passed, 130 tests including four compile-fail doctests; 13 existing optional runtime tests ignored.
- Storage integration coverage before review: `imports` 16 passed, `journal` eight passed, `profiles` six passed on Linux, explicitly wired in Cargo and the existing CI workspace commands.
- `cargo clippy -p feishin-storage -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings`: passed.
- `cargo clippy -p feishin-storage --all-targets --target x86_64-pc-windows-msvc --locked --offline -- -D warnings`: passed cross-compilation.
- `cargo test -p feishin-storage --target x86_64-pc-windows-gnu --tests --locked --offline`: 30 integration tests passed before review under the isolated Wine 11.17 diagnostic, including the native path and reopen/rollback tests. External LLVM-Mingw 20261020 selected LLVM unwind/runtime linkage in place of GCC runtime names as in the foundation diagnostic. This is separate from hosted Windows-MSVC execution and is not a required build tool.
- `python3 tools/verify-inventory.py`: passed. `python3 -m unittest discover -s tools -p 'test_*.py'`: seven passed. `node --check tools/inventory-upstream.cjs`: passed. These ignored local reference tools remain optional evidence checks, not build/CI dependencies.
- `python3 tools/app/check-p00.py`: passed. `python3 -m unittest discover -s tests/tooling -p 'test_*.py'`: 79 ran, 71 passed, eight optional external-reference skips.
- `node --check tests/browser/p02-imports.cjs` and `node --check tests/browser/p02-storage.cjs`: passed.
- Changed Markdown links, migration CSV shape/stable IDs, STATUS length and `git diff --check`: passed.

## Browser runtime

Built the tracked test-only `browser-storage-probe` WASM example after the reviewer-requested current-selection recovery fix and generated bindings with wasm-bindgen-cli 0.2.106. Ran both browser harnesses in a fresh task-owned network-isolated container using the existing external Playwright 1.56.1 lab image, serving only ephemeral loopback assets and fresh browser contexts.

| Engine | Reported version | Existing profile checks | Import journal checks |
| --- | --- | --- | --- |
| Chromium | 141.0.7390.37 | 10 passed | 12 passed |
| Firefox | 142.0.1 | 10 passed | 12 passed |
| WebKit | 26.0 | 10 passed | 12 passed |

The import harness also executes the missing-song-prefix regression through the actual WASM importer and verifies no current selection or timestamp is guessed. It exercises save/reload, one-level rollback/redo across reload, stale-plan rejection, unchanged upstream-named and independent profile keys, quota failure preserving the journal and future-schema protection. Quota/security failures are explicit API fault injection, not exhaustive browser quota/permission certification. The original profile harness was rerun because storage now selects between separate keys. No real export, profile, credentials, network music server or audio is opened by these browser fixtures.

## Review and remaining work

One independent Sol medium reviewer found one P2 issue: after missing-song recovery, raw default-index lookup could select a different occurrence than the source's compacted order and attach the timestamp to it. The target now clears current selection and position whenever default entries are removed or deduplicated. A 15-case regression covers missing prefix/middle/suffix, invalid metadata and duplicate occurrences in disabled/album/empty-track shuffle modes. The browser fixture exercises the missing-prefix case. This stricter corrupt-state policy is recorded in ADR-004.

After the fix, `cargo test -p feishin-storage --locked --offline` passed 31 integration tests (imports17/journal8/profiles6); native/WASM/Windows-MSVC all-target storage Clippy passed, and all 31 storage tests passed as Windows-GNU executables under Wine. The final browser fixture passed profile10/import12 checks per engine. The broader passing workspace suite was not repeated because storage is the only affected production crate; the affected tests and checks above were rerun. The same reviewer completed focused rereview, independently passed the 17 import and eight journal tests and confirmed the P2 resolved with no remaining actionable findings.

The implementation remains partial: other historical versions/transforms, full settings categories and queue metadata, production shell/state-owner adoption, actual resume readiness/seek, credential policy/stores (P02.06) and cache lifecycle (P02.07) remain open. Unsupported imports fail explicitly or report partial projection as documented; no full settings compatibility or music-client behavior is claimed.
