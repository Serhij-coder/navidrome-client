# Historical retained-settings verification

Date: 2026-10-29. Baseline: Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8`. Scope is [retained transforms for settings 14-34](settings-history.md), not complete P02.05 or full settings hydration. The preceding import revision [7047763 passed all seven hosted jobs](https://github.com/Serhij-coder/navidrome-client/actions/runs/40767368341).

## Source evidence

Executed the exact original `settings.store.ts:2442:13` migration method in a controlled VM, together with its original `mergeOverridingColumns` helper and selected exact enums/default initializers. All original files were SHA-256 checked against tracked baseline evidence before tool loading; the method matched inventory SHA-256 `4113f0fd417709d00fbd141e4a4595845dbde7eb2be9c2560b86796958b293ac`. External tools were TypeScript 6.0.3 and Lodash 4.17.21, extracted from official registry packages without lifecycle hooks. They are not application/build/CI dependencies.

The first probe exposed a fixture precondition: the original <=14 method assumes an existing song-list object has a table even when the tested view is detail. The fixture builder was corrected to provide both structural views. Re-running the source probe passed, and a second run matched the saved fixture exactly. No source behavior was changed to accommodate the fixture.

The replay records 273 column outputs across all accepted versions and thirteen list/view combinations, plus 110 Auto DJ outputs, sparse original default merging and duplicate/key-spelling observations. It provides synthetic structural fields for unrelated branches and fixed translation/preset IDs, and compares only the documented retained fields. It does not initialize the application, touch profiles/storage or establish full old-export UI parity. The derived fixture and provenance are tracked under `tests/fixtures/p02/settings-migrations/`; local source and external tools remain optional.

## Exact checks

Used pinned Rust/Cargo 1.92.0 and the established [P02 launcher](README.md#reproduce-checks). No external dependency versions or Cargo lockfile entries changed.

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed.
- `cargo test --workspace --locked --offline`: passed, 144 tests including four compile-fail doctests; 13 existing optional runtime tests ignored.
- Storage coverage: 44 integration tests passed (imports17, journal9, profiles6, settings_migration6, settings_versions6). The aggregate includes the six helper cases, six source/integration matrix cases and the added normalized-preview/apply/rollback case.
- `cargo clippy -p feishin-storage -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings`: passed.
- `cargo clippy -p feishin-storage --all-targets --target x86_64-pc-windows-msvc --locked --offline -- -D warnings`: passed cross-compilation.
- `cargo test -p feishin-storage --target x86_64-pc-windows-gnu --tests --locked --offline`: all 44 integration tests passed under the isolated Wine 11.17/LLVM-Mingw diagnostic described in the [import verification](import-verification.md). Hosted Windows-MSVC execution is separate.
- `node tests/reference/p02-settings-migrations.cjs /tmp/feishin-p02-reference /tmp/feishin-p02-settings-tooling/package/lib/typescript.js /tmp/feishin-p02-settings-lodash/package/lodash.js`: all 273 column and 110 Auto DJ replays matched the tracked fixture; sparse defaults and source observations matched too.
- `python3 -m unittest discover -s tests/tooling -p 'test_p02_settings_replay.py'`: modified-source guard passed; source rejection occurs before missing compiler/library loading or fixture overwrite.

Public helpers initially lacked their own version guard while the wrapper constrained versions. Guards were added before mutation, with regressions for 0/9/11/13/35/u64::MAX and rerun helper/native/WASM checks. The existing import test still expected version 28 to fail; it was updated to unsupported version 35 and rerun alongside all accepted-version integration cases. No failed check was treated as a pass.

## Browser runtime and review

The final test-only WASM example is built with Rust 1.92.0 and wasm-bindgen-cli 0.2.106. The new historical fixture action previews an old alias/column input, checks normalized values, applies that exact reviewed candidate, confirms idempotent reimport and rolls back to the previous target value. Both browser harnesses run in fresh contexts in the task-owned network-isolated external Playwright 1.56.1 lab.

| Engine | Reported version | Profile checks | Import/history checks |
| --- | --- | --- | --- |
| Chromium | 141.0.7390.37 | 10 passed | 13 passed |
| Firefox | 142.0.1 | 10 passed | 13 passed |
| WebKit | 26.0 | 10 passed | 13 passed |

`python3 tools/verify-inventory.py`, seven local reference-tool tests, `node --check tools/inventory-upstream.cjs`, `python3 tools/app/check-p00.py` and changed-script Node syntax checks passed. The full tooling suite ran 80 tests: 72 passed and eight optional external-reference tests skipped. Changed Markdown links and migration CSV shape/actual stable IDs passed; STATUS and staged-diff checks are part of delivery preflight. No ignored source/tools are application build inputs.

One independent Sol medium reviewer independently passed all 44 storage tests, the 273/110 source replay, the tampered-source guard and diff checks. No implementation findings were reported. The sole P3 finding was a stale Auto DJ alias/default deferral in the earlier import map; it was corrected, migration IDs/shape/stale-gap and link/inventory checks passed, and the same reviewer confirmed the finding resolved and review satisfied. No production code changed after passing checks. No full phase, credential, production adoption, device/audio or complete historical-profile claim is made.
