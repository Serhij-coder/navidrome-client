# Playlist replacement backup repository

P03.05 now provides a versioned non-secret recovery repository in feishin-storage::playlist_backups, ahead of destructive replacement orchestration. [ADR-031](../adr/031-playlist-backup-storage.md) records source evidence, limits and deliberate server-scope improvement. The [migration map](../migration/p03-playlist-backups.csv) tracks remaining workflow work.

Repository::load/save/remove use AtomicStore. A schema_version=1 envelope contains revision and backups; each backup contains serverId, playlistId, name, ordered songIds and capturedAtUnixMs (UTC Unix milliseconds). Repeated songs survive. The exact server/playlist pair is the identity, including delimiter characters. Server IDs must be nonblank and at most 128 bytes; playlist/song IDs are nonempty and at most 8192 bytes, names at most 8192 bytes. Timestamps must be at most 8640000000000000. At most 16 scopes, 10000 songs per scope and 256 KiB per encoded document are accepted. A matching scope is replaced; no automatic eviction occurs. Explicit removal frees capacity.

Missing data yields revision-zero empty defaults without writes. Any corrupt/duplicate/future data protects the entire recovery value. Save/remove compare the expected revision and reject exhaustion. These guards require one owner, not multiple concurrent processes/tabs. A native Io failure may follow replacement: reload and reconcile, and do not proceed with destructive server edits on an error.

NativeStore::open_playlist_backups uses <explicit-development-root>/feishin-rust-dev-p02/playlist-backups-v1.json. BrowserStore::open_playlist_backups uses feishin-rust:dev:p03:playlist-backups:v1. Original unscoped keys and independent profile values are untouched. Native repository calls are blocking and require a worker. Browser dispatch uses synchronous localStorage. Async backup ports, native bounded worker composition and the backup/remove/add replacement workflow remain the next increment; direct set_playlist_songs does not provide that workflow.

## Verification

Use Rust/Cargo 1.92.0 and the [established launcher](../p02/README.md#reproduce-checks). Default tests require tracked inputs only:

```sh
cargo test -p feishin-storage --test playlist_backups --locked --offline
python3 -m unittest discover -s tests/tooling -p 'test_p03_playlist_backups.py'
node tools/app/p03-playlist-backup-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
cargo build -p feishin-storage --example browser-storage-probe --target wasm32-unknown-unknown --locked --offline
wasm-bindgen --target web --out-dir target/p03-playlist-backup-web --out-name probe target/wasm32-unknown-unknown/debug/examples/browser_storage_probe.wasm
P02_PLAYWRIGHT=/absolute/playwright node tests/browser/p03-playlist-backups.cjs target/p03-playlist-backup-web
```

Six grouped Rust tests passed for server scope, repeated songs, defaults, revision guards, retention/field/byte limits, partial/future/unscoped recovery protection, exhaustion, failed/uncertain writes, native reopen, interrupted temporary data, path rejection and profile isolation. Three original backup-statement cases passed over normalized inputs; this is not whole-controller parity. The real WASM/localStorage probe passed Chromium, Firefox and WebKit in the existing isolated Playwright 1.56.1 lab, including reload, fresh-context isolation, unchanged original/profile keys, oversized/future/corrupt protection, explicit removal and injected quota/security failures with redacted errors. No live profile or remote mutation was used. Power-loss durability, background browser scheduling, cross-owner coordination and automatic restoration remain unverified.

Combined checks passed on 2026-10-30: 318 workspace tests passed, zero failures, 13 existing optional runtime tests ignored; workspace all-target Clippy, selected portable/WASM all-target Clippy and formatting passed. Tooling passed 112 tests with nine optional skips (121 total). Tracked evidence/links, immutable inventory, seven reference regressions, Node syntax and diff checks passed. Cargo.lock is unchanged. Independent review identified duplicate JSON member collapse through Value. Decoding now reads the original bytes into the typed envelope, and a sixth regression proves duplicate root/record fields cannot discard recovery data. All 108 storage tests and affected native/WASM Clippy passed after the fix; the rebuilt real browser probe passed all three browsers again including duplicate-field protection. The same independent Sol medium reviewer verified the fix and all six targeted tests, with no remaining actionable findings. Linux/WASM only; Windows/macOS excluded. No phase exit is claimed.

The [executor continuation](backup-executor.md) now supplies the typed asynchronous port, bounded native worker and browser bridge. The historical repository-only result above remains separate from backend replacement orchestration.
