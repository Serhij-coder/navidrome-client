# Credential foundation verification

Date: 2026-10-29. Scope: bounded P02.06 library policy and optional Linux provider. See [contract](credentials.md) and [ADR-006](../adr/006-credentials.md). The prior `bfbd9b9` [CI run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40847131827) passed all five jobs before this implementation started. No Windows/macOS builds/tests.

## Automated checks

Pinned Rust/Cargo 1.92.0 with the established P02 launcher. Cargo.lock adds 29 packages for scope hashing/optional Secret Service crypto; all earlier package versions/checksums are unchanged.

- `cargo test --workspace --locked --offline`: 190 passed, including four compile-fail doctests; 13 existing optional runtime tests ignored.
- Feature-on targeted tests: five portable credential cases and two Linux cases passed; the owned GNOME case is ignored in ordinary tests and executed separately below. Feature-off portable tests also passed.
- Workspace fmt/Clippy, optional-feature all-target storage Clippy, and domain/storage/web/remote WASM all-target Clippy passed.
- Final test-only WASM example built. Chromium 141.0.7390.37, Firefox 142.0.1 and WebKit 26.0 each passed five browser policy checks in fresh contexts. Default session values, explicit unsupported remembering, oversized replacement, namespace separation, forget/clear and empty state after reload are exercised; localStorage/sessionStorage stay unchanged. No real authentication data was opened.
- `tools/app/notices.py ... --features feishin-storage/linux-secret-service` collected 402 feature-selected registry notice sets. Dependency notices are required for distributions selecting that feature; the test keyring daemon is not an application bundle.

The portable tests use a non-Send Rc fake provider and known synthetic values to cover explicit persistence, failed remember/forget outcomes, capacity/size replacement, oversized restoration, canonical endpoint identity, invalid inputs and every key dimension. No source credential is imported or emitted in diagnostics.

## Real owned provider

The explicit ignored GNOME fixture passed in an owned network-disconnected Ubuntu 24.04 container with GNOME Keyring 46.1 and D-Bus 1.14.10, built offline with the pinned Rust toolchain and installed system compiler. It used a private D-Bus with no service activation directories and temporary XDG data/config/cache/runtime paths. Synthetic unlock data went only to stdin; no HOME/CODEX_HOME override, live bus, user keyring or prompt interaction occurred.

The final fixture passed after checking read-only readiness, encrypted session store/load, durable restore following daemon restart, replacement, idempotent deletion, locked collection errors for load/write/delete, retained ambiguous entries and untouched foreign app/scope attributes. Cross-collection lookup/update/deletion is exercised using a non-default session collection. Synthetic plaintext scans were performed while data was stored as well as after cleanup. This proves the tested provider path, not every provider's at-rest implementation or secure process-memory erasure.

The first fixture draft retried writes during readiness and scanned only after deletion. Root inspection corrected it to read-only readiness, pre-deletion scans and daemon-restart durability. A private cache directory and five-second bus-start timeout were added, followed by passing affected native tests/Clippy. Ordinary provider errors are fixed categories; no raw provider errors or secret contents are logged.

Tracked P00/link/inventory checks, seven local reference tests, Node syntax, migration CSV/stable IDs and diff checks passed. Tooling ran 82 tests: 74 passed/eight optional skips. One independent Sol medium reviewer reported no actionable findings and approved this bounded increment. The reviewer independently passed seven optional-feature unit cases and diff checking; the real owned provider/browser checks were assessed from recorded evidence. Provider prompt/deadline handling, auth/settings consent/revocation integration and other providers remain open; P02.06/P02 exits are not claimed.

Root inspection also found that default-collection-only lookup could miss an owned item after collection changes. Lookup/update/delete now search exact owned selectors across collections, preserving Locked/Ambiguous errors and using default collection only for creation. The affected private-provider regression was rerun after this fix.
