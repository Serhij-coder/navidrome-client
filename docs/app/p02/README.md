# P02 foundations and bounded persistence

The maintainer [authorized P02.01-P02.04 sequencing](../p01/exit-decision.md) on 2026-10-28 while P00/P01 remain open. This increment implements portable domain foundations and a bounded non-secret profile/preferences store. It does not make the P01 shells a music client or establish any phase exit.

## Implemented behavior

The dependency-free [domain crate](../../../crates/domain/src/lib.rs) contains normalized library models, source sort/filter/feature vocabularies, scoped distinct identities, checked units, correlation/limits/error types and local asynchronous service boundaries. A private bounded owner and deterministic doubles exercise the contracts. See [foundation scope and source evidence](foundation.md) and [ADR-005](../adr/005-portable-contracts.md). P05 queue/scrobble reducers, P03/P04 backend adapters and P06 playback engines are separate work.

The [storage crate](../../../crates/storage/src/lib.rs) provides schema-v1 storage DTOs for up to 64 non-secret server descriptors, selected server and `language`/`follow_system_theme` preferences. These DTOs are deliberately a bounded subset of the normalized domain profile. Defaults match source English and disabled follow-system-theme. Invalid records recover independently, duplicates keep the first valid record, invalid preference fields default with typed diagnostics and orphan selection clears. Unknown schema versions or malformed whole documents fail without writes; explicit saves preserve them rather than replacing them. Recovery reads never rewrite bytes.

Native storage requires an explicitly supplied canonical absolute development root and uses `feishin-rust-dev-p02/profiles-v1.json`. Operations are blocking worker APIs; application worker/executor wiring is not implemented. It syncs a same-directory temporary file before atomic replacement and syncs the directory on Unix. Browser storage uses the independent origin-scoped key `feishin-rust:dev:p02:profiles:v1` with single-value localStorage replacement. Both enforce a 256 KiB document bound. URL credentials/query/fragment, unsupported schemes, duplicate IDs and dangling selection are rejected on save. Plain HTTP LAN server base URLs remain allowed.

No shell opens these stores yet. No original Feishin keys/data are read or modified, and no credential persistence is implemented. See [ADR-004](../adr/004-persistence.md) for recovery, trusted-root/concurrency and filesystem durability limits. To recover an entirely malformed document, preserve/quarantine the original value before explicitly starting a fresh profile. Automated backup/rollback belongs to P02.05.

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

Tests live under [tests/domain](../../../tests/domain/), [tests/storage](../../../tests/storage/) and [tracked fixtures](../../../tests/fixtures/p02/). Manifests explicitly wire integration tests and the test-only WASM example. CI executes native tests on its existing Linux/Windows/macOS matrix and checks all new WASM targets. Compilation is separate from actual browser execution.

Optional browser runtime check, requiring the existing external Playwright 1.56.1 lab/browsers and wasm-bindgen-cli 0.2.106:

```sh
cargo build -p feishin-storage --example browser-storage-probe --target wasm32-unknown-unknown --locked
wasm-bindgen --target web --out-dir target/p02-storage-web --out-name probe target/wasm32-unknown-unknown/debug/examples/browser_storage_probe.wasm
P02_PLAYWRIGHT=/absolute/playwright node tests/browser/p02-storage.cjs target/p02-storage-web
```

The harness serves the test-only WASM bundle on ephemeral loopback in fresh browser contexts. It exercises actual storage across reload, context isolation, unchanged original key, read-only partial recovery and future/malformed protection. Quota and security failures are explicitly injected at the real storage API boundary. It loads no user profile or media. Browser runtime remains an optional external check, not a dependency on ignored source or scripts.

See [exact verification results and limitations](verification.md).

## Remaining bounded tasks

| Task | Status and required follow-up |
| --- | --- |
| P02.01-P02.03 | Foundation implemented and exercised; backend normalization/version discovery and concrete runtime scheduling remain their adapter/integration tasks |
| P02.04 | Bounded native/browser non-secret subset implemented; full settings surface, production shell/worker composition, multi-process/tab coordination and OS/filesystem power-loss evidence remain unverified |
| P02.05 | Historical copied-profile/settings/queue import, column replacement, timestamps, orphan cleanup and backup/rollback fixtures required |
| P02.06 | Explicit remember-credential policy and native/browser secret-store implementations required; only the portable secret boundary exists |
| P02.07 | Server-scoped cache keys, lifecycle/invalidation/cancellation and bounded queue/query/diagnostic serialization required; existing profile byte limits and operation contexts do not complete this task |

The [foundation migration map](../migration/p02-foundation.csv) and [storage migration map](../migration/p02-storage.csv) retain exact source anchors, actual stable inventory IDs where functions exist, verification and gaps. Local reference analysis remains immutable and ignored. No historical import, audio/device or end-to-end parity claim is made.
