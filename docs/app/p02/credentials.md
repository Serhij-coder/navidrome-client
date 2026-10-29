# Bounded credential retention

P02.06 now has a portable credential owner and optional blocking Linux Secret Service adapter in the storage crate. This is a library continuation; the desktop still has no authentication or remember controls. Profile/preferences/import files never receive secret payloads. See [ADR-006](../adr/006-credentials.md) for source retention differences and remaining integration work.

`CredentialKey::new` binds an explicit development namespace, validated profile identity/backend/canonical endpoint, account and slot. Namespace/account lengths are bounded; URL credentials/query/fragment remain rejected. Keys expose a digest attribute, not the original URL/account. Callers must choose a stable isolated namespace for their application data; no root discovery or live Feishin import occurs. Rename preserves a key, while server/account/endpoint/backend/namespace/slot changes separate it.

`CredentialOwner::new()` defaults to session memory. `put_session` and `session` retain/access opaque `Secret` bytes; `clear_session` drops them without deleting OS entries. At most 128 entries and 64 KiB per entry are retained. No provider is contacted by session insertion. The generic owner accepts non-Send backends, preserving browser/local executor compatibility.

`remember` persists an existing session value only on explicit invocation. `restore_remembered` explicitly reads a value and inserts it only after bounds checks. Missing provider values return false; oversized/failing restore preserves existing session data. Failed remember never reports success or silently falls back to another durable store. `forget` clears memory first and attempts provider deletion; failure means an OS copy may remain and must be reconciled. These calls do not implement a sticky remembered setting, logout UI or automatic startup restore.

Default/browser `NoPersistence` returns Unsupported for remember/restore, and session forget succeeds. The browser fixture verifies both origin stores stay byte-for-byte unchanged across operations/reload, including synthetic legacy authentication data. Original keys are neither read as credentials nor silently removed.

Enable Linux support explicitly:

```sh
cargo test -p feishin-storage --features linux-secret-service --test credentials --test linux_credentials --locked
```

`LinuxCredentials::new()` connects lazily during explicit blocking operations. `with_connection` accepts a caller-owned connection, including its method timeout, for private testing/execution. The adapter uses encrypted DH sessions and exact application/development/schema/digest selectors across collections, with the default collection used only for new entries. Locked entries/collections, unavailable providers and duplicate matching entries return typed errors. Missing deletion is idempotent. Foreign application/scope entries remain untouched; labels contain no URL/account/credential. Provider prompts can still occur in the underlying create/delete API; no hard prompt deadline or full async cancellation is claimed.

The optional owned GNOME test requires `dbus-daemon`, `gnome-keyring-daemon` and explicit consent:

```sh
P02_OWNED_SECRET_SERVICE=1 cargo test -p feishin-storage --features linux-secret-service --test linux_credentials owned_secret_service_roundtrip --locked -- --ignored
```

It starts a private bus with service activation disabled and a synthetic passphrase on stdin, uses temporary XDG data/config/cache/runtime directories, and never overrides HOME or connects to a live keyring. It checks durable restore after daemon restart, replacement/deletion, locks, ambiguous matches and foreign attributes. The default feature tests require no keyring; a private unavailable-provider case requires only dbus-daemon. CI runs the optional feature and owned daemon case on Linux.

The [browser harness](../../../tests/browser/p02-credentials.cjs) uses the existing test-only WASM example and fresh contexts. [Verification](credential-verification.md) records commands, actual provider/browser results, notices and limitations. Full credential UI/consent/revocation policy, async worker/prompt integration, backend login/token encoding, other Secret Service providers and P02.07 cache/cancellation remain open. No phase exit or parity claim is made.
