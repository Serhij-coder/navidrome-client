# P03.01 foundation verification

Date: 2026-10-29. Scope: isolated Subsonic request/auth parameter construction from `1a99711`, under the [bounded sequencing exception](README.md). No P02/P03 completion, HTTP login, library operation or live-server parity is claimed. Linux and WASM only; no Windows/macOS builds/tests.

## Source characterization

Read exact archive/inventory evidence for `7d6ad318c79802e226996cfc39acba6b05bb22e8`. The [migration map](../migration/p03-foundation.csv) and [captured fixture](../../../tests/fixtures/p03/subsonic-foundation.json) retain stable IDs and SHA-256 source/function hashes. Frozen upstream qs 6.15.2, md5 2.3.0, charenc/crypt 0.0.2 and is-buffer 1.1.6 tarballs were downloaded without package installers/hooks; each SHA-512 integrity matched the original pnpm lock before controlled extraction. TypeScript 6.0.3 is hash-bound to the established reference compiler.

The original normalize/getServerUrl/randomString/authenticate/parsePath/request-callback/serializer expressions ran in the controlled tool with synthetic acknowledgements and a fake Axios boundary. Eight cases passed: special salted, special legacy, Unicode, empty salted, empty legacy, literal-percent username, already-rest and double trailing slash. Seven constructed wire URLs match Rust byte-for-byte, including repeat keys, empty values, RFC3986 punctuation, spaces/plus and Unicode. The eighth records the original saved-request URIError and the target's deliberate raw-username correction. No socket, live service, live credential profile or original installer/script was used. No real server is asserted to accept the synthetic authentication result.

## Local checks

Rust/Cargo 1.92.0 used the [established launcher](../p02/README.md#reproduce-checks) and `CARGO_TARGET_DIR=/home/serhii/Projects/feishin-subsonic-foundation-target`. All checks below passed after the final implementation/fixture changes:

- `cargo fmt --all -- --check`.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`.
- `cargo test --workspace --locked --offline`: 197 passed, including seven new adapter tests and four existing compile-fail doctests; 13 existing optional runtime tests ignored.
- `cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings`.
- Focused native and WASM `cargo check -p feishin-subsonic --all-targets --locked --offline` (with `--target wasm32-unknown-unknown` for WASM), adapter tests and focused Clippy also passed during implementation.
- `python3 tools/app/check-p00.py`: tracked evidence and maintained Markdown links passed. `git diff --check` passed.
- `python3 -m unittest discover -s tests/tooling -p 'test_*.py'`: 85 total, 76 passed/nine optional external-reference skips. The three P03 tooling tests passed separately with the supplied pinned reference paths, including the eight-case replay. Node syntax checking for the new replay tool passed.
- In the unchanged local reference environment: `python3 tools/verify-inventory.py`, seven ignored-reference-tool regression tests and `node --check tools/inventory-upstream.cjs` passed. All nine migration rows were checked against stable inventory IDs; the CSV schema and link checks passed after correcting the callback's recorded end line to 612.
- `python3 tools/app/notices.py /home/serhii/Projects/feishin-subsonic-foundation-target/notices`: 379 feature-selected registry notice sets collected, including md-5 license/manifests. Comparison with `1a99711:Cargo.lock` confirms every existing package entry is unchanged; only local feishin-subsonic and md-5 0.10.6 were added.

One independent Sol medium reviewer inspected the complete scope, including exact extracted TS source, and reported no actionable findings. The reviewer independently passed seven adapter tests, all three P03 tooling tests (including the eight-case original-expression replay), tracked P00/documentation checks and `git diff --check`. No review fixes were requested. Hosted CI is observed after push and reported with the delivered commit; no unobserved future CI outcome is asserted by this record.

## Remaining evidence

No actual network request, login response, transport error, provider, browser runtime, playback, UI or packaged adapter integration was exercised. Fixed constructor errors/redacted Debug and typed parameter generation do not establish redaction in a future HTTP client's logs or nested errors. Entropy tests prove mapping/failure/bounds with deterministic doubles, not the strength or availability of an unimplemented platform entropy provider. Existing Linux AppImage CI checks certify their tested shell/probe scope; this new library is not wired into a packaged application. P02/P03 and earlier open gates remain open.

## Integration onto main

The maintainer requested integration of `p03-subsonic-foundation` at `c877070` onto main at `371afa4`. The branch had a clean worktree and [all five CI jobs passed](https://github.com/Serhij-coder/navidrome-client/actions/runs/40877068394). A normal merge retains P02.06 executor and P02.07 cache work. The only conflict was adjacent additions to the application guide; both sections are retained. No adapter/domain/storage logic or dependency version changed during assembly. Combined pinned-toolchain checks passed: workspace fmt; workspace all-target Clippy; workspace tests (220 passed including four compile-fail doctests, 13 optional runtime tests ignored); domain/storage/Subsonic/web/remote all-target WASM Clippy; tracked P00/link checks; tooling tests (76 passed/nine optional skips); seven local reference regressions; inventory integrity and both Node syntax checks. All nine new migration IDs validate, and lock comparison confirms only the local adapter and md-5 0.10.6 were added with existing entries unchanged. The branch's previously reviewed eight-case original-expression replay is unchanged and was not repeated during assembly. The one independent final Sol medium integration reviewer found no actionable findings. It independently passed seven adapter tests, P00/link checks, two tooling tests and staged whitespace checks; the optional external replay remained skipped with unchanged prior evidence retained. Integration review gate satisfied. Linux/WASM scope and all recorded limitations remain in force.
