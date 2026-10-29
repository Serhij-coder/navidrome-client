# Album detail verification

2026-10-29. Bounded P03.03 album/album-artist reads, implemented by the sole primary orchestrator following the maintainer's instruction. The required independent final reviewer remains separate. No phase exit is claimed.

The pinned toolchain is Rust/Cargo 1.92.0, using the [established launcher](../p02/README.md#reproduce-checks). Source comes from the immutable local archive/inventory at `7d6ad318c79802e226996cfc39acba6b05bb22e8`. The new fixture stores four file hashes and 14 original expression hashes; source controllers, normalizers and enums are executed only after verification, with in-memory API responses. No original module startup, installer, package hook, profile access or network call runs.

```sh
cargo test -p feishin-subsonic --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
node tools/app/p03-album-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
```

Focused adapter tests:32 passed (four album, ten existing song, 11 protocol, seven foundation). Combined workspace:252 passed including four compile-fail doctests; 13 existing optional runtime tests ignored. Workspace/native and selected WASM all-target Clippy passed. Source replay:18 cases passed, covering 11 projected source successes, five explicit typed rejections of malformed source successes and two source TypeErrors. Existing song fixtures passed after shared-helper extraction. No dependency or Cargo.lock change was needed.

The four grouped album cases test every emitted normalized field, nested work limits, duplicate discs and repeated songs, scoped identities, malformed data, stale results, HTTP/protocol/network errors, byte limits, pending cancellation and final context priority over both success and malformed/mismatched results. Tests use non-Send Cell/RefCell doubles and explicit polling, not a server or executor. Shared test projections now support existing song and new album comparisons.

Default provenance/tampering guards pass. Formatting, tracked P00/link/inventory checks (767 local links), full tooling (80 passed/nine optional skips), seven local reference regressions and Node syntax checks passed. All six migration IDs and 14 replay expression IDs match the immutable inventory; Cargo.lock is unchanged. After expanding the fixture to 18 cases, affected adapter tests and native/WASM adapter Clippy were rerun. The one independent Sol medium reviewer found no actionable findings across actual tracked and new files, source behavior, architecture, tests, performance bounds and operations. It independently passed all four album integration tests, 18 original-controller replay cases and two provenance/tampering tests. Hosted CI will be observed after review and push. No live native/browser transport, browser runtime, server conformance, packaged adapter or audio/device result is established here.

Reference replay requires the explicit source root (four fixture-listed files, original paths) and the hash-matched compiler. Those are optional local evidence inputs only; default builds/CI consume tracked Rust/tests/fixtures. The script's optional --write mode regenerates expected observations and must be reviewed and tested. See [ADR-016](../adr/016-subsonic-album-reads.md) for the precise projection and validation differences.
