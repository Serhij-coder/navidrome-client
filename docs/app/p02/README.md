# P02 foundations and bounded persistence

The maintainer [authorized P02.01-P02.04 sequencing](../p01/exit-decision.md) on 2026-10-28 while P00/P01 remain open. The maintained implementation includes portable domain foundations, bounded non-secret profile/preferences storage and a bounded P02.05 copied-input preview/apply/rollback continuation. It does not make the P01 shells a music client or establish any phase exit.

## Implemented behavior

The dependency-free [domain crate](../../../crates/domain/src/lib.rs) contains normalized library models, source sort/filter/feature vocabularies, scoped distinct identities, checked units, correlation/limits/error types and local asynchronous service boundaries. A private bounded owner and deterministic doubles exercise the contracts. See [foundation scope and source evidence](foundation.md) and [ADR-005](../adr/005-portable-contracts.md). P05 queue/scrobble reducers, P03/P04 backend adapters and P06 playback engines are separate work.

The [storage crate](../../../crates/storage/src/lib.rs) provides schema-v1 storage DTOs for up to 64 non-secret server descriptors, selected server and `language`/`follow_system_theme` preferences. These DTOs are deliberately a bounded subset of the normalized domain profile. Defaults match source English and disabled follow-system-theme. Invalid records recover independently, duplicates keep the first valid record, invalid preference fields default with typed diagnostics and orphan selection clears. Unknown schema versions or malformed whole documents fail without writes; explicit saves preserve them rather than replacing them. Recovery reads never rewrite bytes.

Native storage requires an explicitly supplied canonical absolute development root and uses `feishin-rust-dev-p02/profiles-v1.json`. Repository operations are blocking APIs; the later NativeWorker continuation runs them on a dedicated thread. Shell consumption and background browser scheduling remain open. It syncs a same-directory temporary file before atomic replacement and syncs the directory on Unix. Browser storage uses the independent origin-scoped key `feishin-rust:dev:p02:profiles:v1` with single-value localStorage replacement. Both enforce a 256 KiB document bound. URL credentials/query/fragment, unsupported schemes, duplicate IDs and dangling selection are rejected on save. Plain HTTP LAN server base URLs remain allowed.

The [opt-in desktop preferences](desktop-preferences.md) now consume the bounded profile/preferences subset through the storage worker. Browser/remote shells do not open these stores. No original Feishin keys/data are read or modified, and credential persistence is available only through the separate explicit opt-in library described below. See [ADR-004](../adr/004-persistence.md) for recovery, trusted-root/concurrency and filesystem durability limits. To recover an entirely malformed document, preserve/quarantine the original value before explicitly starting a fresh profile. The separate import workspace now provides bounded one-level target backup/rollback; it does not automatically repair the profile store.

## Reproduce checks

Use pinned Rust 1.92.0 with rustfmt, Clippy and wasm32-unknown-unknown from the [P01 setup](../p01/README.md#setup-and-build). This host uses the already-established launcher:

```sh
export PATH="/tmp/feishin-p01-cargo/bin:$PATH"
export CARGO_HOME=/tmp/feishin-p01-cargo
export RUSTUP_HOME=/tmp/feishin-p01-rustup
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo clippy -p feishin-domain -p feishin-storage --all-targets --target wasm32-unknown-unknown --locked -- -D warnings
cargo clippy -p feishin-web -p feishin-remote --target wasm32-unknown-unknown --locked -- -D warnings
```

Tests live under [tests/domain](../../../tests/domain/), [tests/storage](../../../tests/storage/) and [tracked fixtures](../../../tests/fixtures/p02/). Manifests explicitly wire integration tests and the test-only WASM example. At the maintainer's direction, CI and current build verification cover Linux and WASM/browser targets; Windows and macOS builds/tests are excluded. Linux AppImage checks retain x86_64 and aarch64. Historical platform results remain evidence of earlier runs. Compilation is separate from actual browser execution.

Optional browser runtime check, requiring the existing external Playwright 1.56.1 lab/browsers and wasm-bindgen-cli 0.2.106:

```sh
cargo build -p feishin-storage --example browser-storage-probe --target wasm32-unknown-unknown --locked
wasm-bindgen --target web --out-dir target/p02-storage-web --out-name probe target/wasm32-unknown-unknown/debug/examples/browser_storage_probe.wasm
P02_PLAYWRIGHT=/absolute/playwright node tests/browser/p02-storage.cjs target/p02-storage-web
```

The harness serves the test-only WASM bundle on ephemeral loopback in fresh browser contexts. It exercises actual storage across reload, context isolation, unchanged original key, read-only partial recovery and future/malformed protection. Quota and security failures are explicitly injected at the real storage API boundary. It loads no user profile or media. Browser runtime remains an optional external check, not a dependency on ignored source or scripts.

See [foundation verification](verification.md), [bounded import verification](import-verification.md), [historical transform verification](settings-history-verification.md) and [early/reset verification](early-settings-verification.md) for exact results and limitations.

## Remaining bounded tasks

| Task | Status and required follow-up |
| --- | --- |
| P02.01-P02.03 | Foundation implemented and exercised; backend normalization/version discovery and concrete runtime scheduling remain their adapter/integration tasks |
| P02.04 | Bounded native/browser non-secret subset implemented; full settings surface, shell composition/background browser scheduling, multi-process/tab coordination and OS/filesystem power-loss evidence remain unverified |
| P02.05 | Bounded copied-input projection and atomic target backup/rollback implemented; unsupported fields/transforms, full settings/queue metadata, real historical-profile corpus and production adoption remain open |
| P02.06 | Bounded session owner/explicit retention APIs and optional Linux Secret Service adapter implemented; browser remembering explicitly Unsupported. Login/UI consent, durable consent/revocation reconciliation and provider-aware prompt handling remain open; bounded portable/native executor implemented |
| P02.07 | Bounded server/library-scoped keys, correlated lifecycle, per-server/family invalidation and fixed diagnostic implemented; concrete transport/UI wiring, unmodeled keys and full runtime queue/query serialization remain open |

The [foundation migration map](../migration/p02-foundation.csv) and [storage migration map](../migration/p02-storage.csv) retain exact source anchors, actual stable inventory IDs where functions exist, verification and gaps. Local reference analysis remains immutable and ignored. No full historical import, audio/device or end-to-end parity claim is made. See the [supported import matrix](imports.md) and [apply/rollback contract](import-journal.md).

The [historical settings continuation](settings-history.md) expands retained-field transforms to versions 14-34, with exact pinned source replay and current/backup transaction coverage.

The [early/reset continuation](early-settings.md) completes retained settings version handling for 0-34, keeping reset scope and rollback explicit.

The [active-state repository](state-adoption.md) adds explicit staged-projection replacement, legacy-v1 bootstrap, stale guards and one-value rollback. The native worker now wraps this durable boundary; shell composition and background browser scheduling remain open; [verification](state-adoption-verification.md) separates repository and browser results from runtime application integration.

The [storage worker continuation](state-worker.md) adds typed session dispatch, generation-scoped single-use review tokens and a bounded native thread. Browser local dispatch is exercised without a Web Worker claim. Shell/UI composition remains open; [verification](state-worker-verification.md) records exact results and lifecycle limits.

The [native preferences continuation](desktop-preferences.md) wires explicit isolated storage into the desktop for two preference fields, with revision-checked worker patches and cooperative close. Full import/profile-management/browser consumption remains open; [verification](desktop-preferences-verification.md) records native input and recovery evidence.

The [credential library continuation](credentials.md) adds session-only default ownership, explicit remember/restore/forget and optional Linux Secret Service, preserving non-secret profile/journal schemas. It remains separate from application authentication/remember controls; [verification](credential-verification.md) records owned provider/browser evidence and remaining gates.

The [query cache continuation](query-cache.md) and [ADR-007](../adr/007-query-cache.md) define portable bounded identity/lifecycle rules; [verification](query-cache-verification.md) records combined executor integration and actual three-browser execution. The [credential executor](credential-executor.md) runs native provider work off the caller thread without claiming hard prompt cancellation or shell consumption.
