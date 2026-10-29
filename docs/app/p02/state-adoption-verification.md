# Active-state adoption verification

Date: 2026-10-29. Scope: P02.05 durable repository boundary and maintainer-directed Linux/WASM build-test scope. See [the contract](state-adoption.md). Production shell/worker composition, queue reducer and playback restoration are not delivered.

## Environment and checks

Rust/Cargo 1.92.0, using the established launcher in the [P02 guide](README.md#reproduce-checks). No dependency/lockfile changes. Windows/macOS builds/tests were excluded; existing platform files and historical records were preserved. The prior `c4e6505` [hosted run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40828231313) completed successfully; its former platform matrix predates this instruction.

- `cargo test --workspace --locked --offline`: 160 passed including four compile-fail doctests; 13 existing optional runtime tests ignored. This included the first seven state tests. After adding rollback-failure coverage and strengthening native adoption/reopen coverage, the affected state target passed eight tests. Following the review bounds fix, `cargo test -p feishin-storage --tests --locked --offline`: all 62 integration tests passed, including nine state tests.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed after the review bounds fix and final state tests.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p feishin-domain -p feishin-storage -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings`: passed.
- `cargo build -p feishin-storage --example browser-storage-probe --target wasm32-unknown-unknown --locked --offline`: passed. The initial browser-probe snapshot/data type mismatch was corrected, then the final WASM example built successfully and executed below.

The nine state tests cover read-only defaults/recovery and explicit first persistence, first-adoption legacy backup/undo/redo, exact source/target/initial-profile guards, local owner saves, ignored legacy corruption after activation, two occurrences of one song, invalid/future/oversized active documents, invalid backup, revision exhaustion, current-plus-backup capacity, pre-replacement errors, visible-replacement durability errors and native three-namespace reopen/isolation. Native execution uses only a task-owned temporary root; interrupted temporary data does not replace the active snapshot. Adoption and rollback leave the real stage/profile bytes unchanged.

## Browser execution

The final WASM example was converted with wasm-bindgen-cli 0.2.106. A task-owned network-isolated `localhost/agent-browser-lab:2026-10-22` container ran Playwright 1.56.1. No existing user browser/profile or media was opened. The [state harness](../../../tests/browser/p02-state.cjs) ran 18 assertions in each fresh context:

| Engine | Version | Result |
| --- | --- | --- |
| Chromium | 141.0.7390.37 | 18 passed |
| Firefox | 142.0.1 | 18 passed |
| WebKit | 26.0 | 18 passed |

Assertions include read-only legacy bootstrap, staged adoption, reload, idempotence, initial undo/redo, local save, stale target/stage rejection, unchanged legacy/original keys, stripped synthetic credentials, injected quota failure and malformed/future active protection. Storage/reload use real browser APIs; quota failure is explicitly injected and is not a naturally exhausted disk. Browser/OS certification and multi-tab writer coordination are not established.

Before the review fix, the bundle also passed the existing profile (10 assertions) and import/reset (17 assertions) harnesses in each engine. Their paths were unchanged by the local-save bounds fix. After that fix the rebuilt WASM passed all 18 active-state assertions again in each engine. Both task-owned containers were removed after checks.

## Documentation, review and delivery

Tracked P00/link checks, immutable inventory verification, seven local reference-tool tests, Node syntax checks and `git diff --check` passed. The full tracked tooling suite ran 80 tests: 72 passed, eight optional external-reference tests skipped. The first documentation check ran before this linked verification file existed; after adding it, the same check passed. Adoption migration CSV structure and stable inventory IDs were checked. One independent Sol medium reviewer found a P2 bounds issue: local saves cloned caller data before validation. Borrowed data now passes validation before cloning; internal validation is retained. A regression rejects an oversized typed queue field without changing active data. All 62 storage tests, workspace Clippy/fmt and native/WASM checks passed after the fix. The same reviewer confirmed the fix resolved, independently reran all nine state tests and diff checking, and reported no remaining actionable findings. Final review is satisfied. No phase exit or parity claim follows from these tests. P02.06 credential policy/platform stores and P02.07 cache/cancellation remain open.
