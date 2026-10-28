# P02 foundation and bounded persistence verification

Date: 2026-10-28. Baseline: Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8`. All builds/tests use tracked inputs; source evidence is read only for migration traceability. P00/P01 and the P02 phase exit remain open.

## Local checks

The pinned host launcher in the [P02 guide](README.md) selected Rust/Cargo 1.92.0. Native Linux and wasm32-unknown-unknown targets were installed. No dependency versions were upgraded; the lockfile adds only the two workspace packages and their existing pinned dependencies.

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed after removing an unused native function from the test-only WASM example.
- `cargo test --workspace --locked --offline`: passed, 104 tests including four compile-fail doctests, 13 existing optional tests ignored. A subsequent storage regression increased its integration target from five to six tests; `cargo test -p feishin-storage --locked --offline` passed all six after that change.
- `cargo clippy -p feishin-domain -p feishin-storage -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings`: passed. The updated storage test target was also rechecked on native and WASM.
- `cargo check -p feishin-domain -p feishin-storage --all-targets --target x86_64-pc-windows-msvc --locked --offline`: passed cross-compilation; no Windows execution claimed.
- Domain target: nine deterministic integration tests plus four compile-fail doctests passed. Storage target: six integration tests passed.
- `python3 tools/verify-inventory.py`: passed exact archive/inventory/link integrity checks. `python3 -m unittest discover -s tools -p 'test_*.py'`: seven passed. `node --check tools/inventory-upstream.cjs`: passed. These ignored reference tools are not build/CI dependencies.
- `python3 tools/app/check-p00.py`: passed. `python3 -m unittest discover -s tests/tooling -p 'test_*.py'`: 79 ran, 71 passed and eight optional external-reference tests skipped.
- `node --check tests/browser/p02-storage.cjs`: passed. Migration CSV stable IDs/shape and changed local Markdown links checked separately; passed after correcting two source-function line references.

## Browser storage execution

Built the tracked `browser-storage-probe` WASM example using the pinned compiler and wasm-bindgen-cli 0.2.106. Ran `tests/browser/p02-storage.cjs` against that generated bundle in a fresh task-owned container made from the existing external `agent-browser-lab:2026-10-22` image, Playwright 1.56.1, with no external network. The fixture serves only loopback test assets and uses fresh browser contexts.

| Engine | Reported version | Result |
| --- | --- | --- |
| Chromium | 141.0.7390.37 | All ten assertions passed |
| Firefox | 142.0.1 | All ten assertions passed |
| WebKit | 26.0 | All ten assertions passed |

Assertions cover default data, save, reload, unchanged upstream-named fixture key, context isolation, quota failure retaining the prior value, storage security failure, future-schema save protection, malformed JSON save protection and read-only partial recovery. Quota/security errors are explicitly injected through Storage methods; they are not a claim of exhaustive browser quota/permission certification. Real browser storage is executed; audio, UI, mobile/device and backend traffic are not exercised.

## Review and limitations

One independent Sol medium reviewer completed the combined review and independently passed 15 integration tests plus four compile-fail doctests. No actionable implementation findings were found. The sole P3 finding was a stale paragraph in the P01 next-work guide claiming that sequencing approval was pending. It was corrected, inventory/link/stale-wording checks were rerun successfully and the same reviewer confirmed the finding resolved and final review satisfied. No implementation changed after the passing checks.

The first [hosted run for dea4b4f](https://github.com/Serhij-coder/navidrome-client/actions/runs/40758200837) passed portable/tooling, WASM, native Linux/macOS and both AppImage jobs but failed Windows on two new tests. Public check annotations identified the catalog fixture comparison and native storage roundtrip; full logs required authenticated access (HTTP 403).

The catalog test compared raw LF output with a fixture subject to CRLF checkout conversion. A deterministic LF/CRLF regression reproduced that defect and now checks exact records without normalizing vocabulary/whitespace. Native storage inspected a Windows drive/device prefix before RootDir made it a valid filesystem path. An isolated Windows-GNU/Wine diagnostic reproduced the same roundtrip failure with Error::Io and showed symlink_metadata rejecting the incomplete verbatim prefix. The fix postpones that metadata check until the rooted path, while retaining checks of the root and every subsequent component. A Windows-specific regression verifies ordinary and canonical verbatim paths reopen/replace the same value.

After both fixes, `cargo test -p feishin-domain -p feishin-storage --locked --offline` passed ten domain plus six storage tests and four compile-fail doctests. Both crates passed native, WASM and Windows-MSVC all-target Clippy with `-D warnings`; formatting passed. The original failing storage test plus its Windows-specific regression and all other new tests also passed as Windows-GNU executables under Wine 11.17 (16 integration tests). This external diagnostic used Rust 1.92.0, LLVM-Mingw 20261020 with LLVM unwind/runtime linkage instead of GCC runtime names and an isolated `/tmp/feishin-p02-wine` prefix; none is a build/CI dependency. Production Windows-MSVC hosted confirmation is separate. Browser code was unchanged by these native/test-only fixes, so its passing runtime checks were not repeated. The same independent Sol medium reviewer inspected both corrections, independently passed 16 integration tests plus four compile-fail doctests and confirmed focused rereview satisfied with no actionable findings.

Linux tests and browser lab results do not establish macOS/Windows runtime, filesystem power-loss guarantees, concurrent writers, historical imports/rollback, credential remembering, cache invalidation, production queue/engine behavior or phase completion. Native/browser storage is not wired into the P01 shells. See the [remaining tasks](README.md#remaining-bounded-tasks), [foundation map](../migration/p02-foundation.csv), [storage map](../migration/p02-storage.csv) and [sequencing decision](../p01/exit-decision.md).
