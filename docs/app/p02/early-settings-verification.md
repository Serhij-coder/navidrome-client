# Early/reset settings verification

Date: 2026-10-29. Scope: explicit settings versions 0-13 and retained reset handling, completing numeric versions 0-34 for the partial schema. See the [contract and remaining gaps](early-settings.md). Previous revision [0121a86 completed all seven hosted jobs](https://github.com/Serhij-coder/navidrome-client/actions/runs/40791476724).

## Source characterization

The optional pinned-source replay still reproduces all existing 273 column and 110 Auto DJ outputs. Its `--early` mode executes the same exact original migration/default merge for 56 synthetic cases across versions 0-13 and captures five malformed observations. All five original files and the method SHA-256 are verified before compiler loading/evaluation. Exact early window/playback/general default initializers were added to the isolated context; no original module initialization, profile/storage adapter or package hook executes.

Captured behavior distinguishes: 0-9 early return, v8 sidebar mutation and Linux window-bar choice, 10/11 empty returned state, and 12/13 later transforms. Three malformed v10 cases throw before reset; two null v11 cases return an empty state. The target deliberately rejects all five malformed typed copies rather than turning bad input into a reset. Retained defaults match the pinned source, while empty target column overrides mean removed customizations, not complete original UI default columns.

The generated fixture is synthetic/source-derived, not an older real-user export corpus. Mixed-copy authoritative/supplemental reset precedence is an explicit target partial-import policy and is tested separately rather than claimed as original multi-snapshot behavior.

## Exact checks

Pinned Rust/Cargo 1.92.0 with the [existing launcher](README.md#reproduce-checks). No external dependency versions or lockfile entries changed.

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed.
- `cargo test --workspace --locked --offline`: 153 passed including four compile-fail doctests; 13 existing optional runtime tests ignored.
- Storage integration: 53 passed (imports17, journal11, profiles6, settings_migration8, settings_versions6, early_versions5). The five new source/projection tests exercise all 56 combined/split cases and malformed/mixed-copy invariants.
- `cargo clippy -p feishin-storage -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings`: passed.
- `cargo clippy -p feishin-storage --all-targets --target x86_64-pc-windows-msvc --locked --offline -- -D warnings`: passed cross-compilation.
- `cargo test -p feishin-storage --target x86_64-pc-windows-gnu --tests --locked --offline`: all 53 integration tests passed under the isolated Wine 11.17/LLVM-Mingw diagnostic described in the [import verification](import-verification.md). This is separate from hosted Windows-MSVC execution.
- Existing optional replay command with external TypeScript6.0.3/Lodash4.17.21: 273/110 cases matched. The same command with `--early` matched all 56 cases/five malformed observations. Both consume verified local source but are not application/default CI dependencies.
- `python3 -m unittest discover -s tests/tooling -p 'test_p02_settings_replay.py'`: modified-source rejection passed in both normal and early output modes, before compiler/library loading or either fixture write.

The first helper test compile failed while the new reset error variant was not yet present; after the root added it, the rerun passed helper8 tests and native/WASM checks. The prior import test's unsupported v13 expectation was changed to future v36 and rerun with the complete 0-13 source matrix; accepted numeric versions were not tested only by widening an allowlist.

## Browser runtime and delivery checks

The test-only WASM example was rebuilt with the final implementation and wasm-bindgen-cli0.2.106. Fresh browser contexts in a task-owned network-isolated Playwright1.56.1 lab executed the existing profile harness and the expanded import harness. New assertions cover malformed-reset rejection, explicit reset save, reload of defaults and exact backup restoration. Existing historical, stale, quota and future-schema checks remain included; quota/security faults are explicit injection, distinct from real storage execution.

| Engine | Reported version | Profile checks | Import/reset checks |
| --- | --- | --- | --- |
| Chromium | 141.0.7390.37 | 10 passed | 17 passed |
| Firefox | 142.0.1 | 10 passed | 17 passed |
| WebKit | 26.0 | 10 passed | 17 passed |

Inventory verification, seven local reference-tool tests, Node syntax checks and tracked P00 evidence checks passed. The full tooling suite ran 80 tests: 72 passed, eight optional external-reference tests skipped. Migration IDs/CSV shape and changed local links passed. A trailing blank line flagged by diff checking was removed; diff and the affected replay-guard test were rerun. One independent Sol medium reviewer passed all 53 storage integration tests, both replay tamper-guard modes and diff checks. No implementation findings were reported. The sole P3 finding was an older ADR paragraph still calling versions below 14 unsupported; it was updated to the current early/reset/transform contract. The stale-coverage regression, affected links, inventory and diff checks passed; the same reviewer confirmed the finding resolved and final review satisfied. No implementation changed after passing checks. No full settings hydration, production adoption, credential/cache policy, device/audio parity or P00/P01/P02 exit is claimed.
