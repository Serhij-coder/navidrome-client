# Asynchronous playlist backup boundary

P03.05 adds a usable PlaylistBackupStore boundary before destructive replacement. [ADR-032](../adr/032-playlist-backup-executor.md) records scheduling/error semantics and alternatives; [migration map](../migration/p03-backup-executor.csv) separates implementation from remaining integration.

The domain model carries scoped playlist identity, name, ordered/repeated SongIds and explicit UnixMilliseconds. Save returns a context-correlated durable revision or typed failure with may_have_written. No failed capture authorizes removal. NativePlaylistBackups::open requires the explicit development root; with_factory constructs even non-Send stores on its dedicated thread. There is exactly one admitted request, including an abandoned in-flight request. A following save receives Capacity until that work completes. is_idle reports admission state, not successful persistence. begin_shutdown stops admission; try_join is nonblocking; Drop never joins an OS call.

The worker checks cancellation/latest start before storage work after the preliminary read and immediately before replacement after the final read/encoding. Save futures also check the caller context on each poll and on completion. Caller scheduling must wake for cancellation/deadline; a blocked OS write is not forcibly interrupted. Dropping a save cancels pending work but cannot undo a started write. Native Io, panic and pending context failures can carry may_have_written=true; reload/reconcile instead of automatic retry. The single-owner revision check does not coordinate multiple processes/tabs.

BrowserPlaylistBackups::open uses the isolated browser repository and a local non-Send future. Dispatch remains synchronous localStorage. The item budget and request-byte limit bound capture; actual escaped full-document serialization runs on the native worker, capped at the smaller of that limit and 256 KiB. Repository recovery reads retain their fixed 256 KiB cap; no response-byte truncation of recovery data occurs. Defaults, corruption/future/duplicate protection and other scopes remain unchanged.

## Verification

Use the [pinned launcher](../p02/README.md#reproduce-checks):

```sh
cargo test -p feishin-storage --test backup_worker --test playlist_backups --locked --offline
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
```

Nine worker tests exercise real native reopen/scoping/repeated IDs, exact receipts, non-Send factory/store placement, preflight failures without I/O, escaped-byte overflow, abandoned request backpressure, cancellation before write, pending deadline/uncertain writes, actual completion wakeups, latest-start expiry after blocked factory construction, provider errors/panic and nonblocking Drop. The existing six repository regressions also pass. The rebuilt [real WASM browser probe](../../../tests/browser/p03-playlist-backups.cjs) passed Chromium, Firefox and WebKit in the existing isolated Playwright 1.56.1 lab, now including typed-port save/reload, cancellation/deadline/tiny-byte no-write results and redacted injected quota failures. See [repository reproduction](playlist-backups.md) for bundle/harness commands. This is library/storage evidence, not production scheduling or server mutation parity.

Combined checks passed on 2026-10-30: 327 workspace tests passed, zero failures, 13 existing optional runtime tests ignored; workspace and selected portable/WASM all-target Clippy, formatting, 112 tooling tests with nine optional skips, tracked evidence/links, inventory integrity, seven reference regressions, Node syntax and diff checks passed. The panic/queued-request cleanup refinement was followed by passing all 14 worker/repository tests and affected all-target Clippy. No dependency or Cargo.lock change. Independent review found a second-read cancellation gap. The guarded repository save now checks immediately before replacement, and a ninth regression blocks the final read to verify dropped-request cancellation and native latest-start expiry prevent writes. All 117 storage tests and affected native/WASM all-target Clippy passed after the fix. The same independent Sol medium reviewer verified the guard and regression, with no remaining actionable findings. The rebuilt three-browser probe also passed after the fix. The actual backup/remove/add backend workflow, interruption/partial server outcomes, shell adoption, live-server conformance and all phase exits remain open. Linux/WASM only; Windows/macOS excluded.
