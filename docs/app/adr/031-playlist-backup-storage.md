# ADR-031: isolated versioned playlist replacement backups

Status: accepted for bounded P03.05 storage foundation, 2026-10-30. Replacement orchestration and phase exit remain open.

## Evidence and decision

Pinned Feishin 7d6ad318c79802e226996cfc39acba6b05bb22e8 replacePlaylist at src/renderer/api/subsonic/subsonic-controller.ts:2212:22 awaits an idb-keyval write before removing reversed old positions and appending replacements. Its unversioned playlist-backup-${id} key omits server identity. Three cases execute only the exact backup declarations and awaited set statement over normalized synthetic inputs, with hashes for the source, parent expression, selected expressions and compiler. They demonstrate the colliding source key, repeated IDs/order, Unix milliseconds and rejected writes. They do not execute the whole replacement workflow or normalization.

Reuse feishin-storage and AtomicStore, adding a small playlist_backups module and distinct native/browser values. A schema-v1 document stores a checked revision and at most 16 latest backups, each keyed by the exact server/playlist pair. Preserve song order and repeated IDs. Store non-secret name, IDs and explicit captured_at_unix_ms. The monotonic operation clock must never supply this timestamp. The one-value document avoids filename/key injection from IDs and permits existing atomic replacement and bounded serialization. No dependency or crate is added.

Reject records exceeding 10,000 IDs, the 256 KiB encoded envelope, field limits or timestamp range. Read-only defaults do not write. Any corrupt record, duplicate scope, unsupported version or malformed document protects the entire value against save/remove. Never silently discard recovery data. Expected revisions and checked increments guard a single owner; this is not inter-process/tab CAS. Replacing one matching scope retains other scopes; only explicit removal frees capacity, with no automatic eviction. Missing-scope removal is a no-op.

## Alternatives and consequences

Reusing the unscoped source key risks overwriting another server's recovery data. Automatic historical import is unsafe because the original document cannot establish its server identity; it is rejected rather than guessed. Keep original keys and profile values untouched. A database or separate file per backup adds a new boundary before the bounded feature needs it. A single capped document has explicit capacity failures; larger histories and coordination require a later design.

Native writes inherit private temporary-file replacement, file synchronization and Linux directory synchronization. An Io error can occur after replacement, so reload/reconciliation is required; never treat a failed save as permission to remove songs. Browser localStorage replacement is synchronous and quota/security failures remain typed and redacted. Neither adapter proves power-loss durability. Native methods are blocking worker APIs; no async native worker, backup application port or destructive backend workflow is included in this increment. Those are the next bounded task. No automatic rollback/retry, transaction across storage and server, production adoption or live-server parity is claimed.

See [contract and verification](../p03/playlist-backups.md) and [migration map](../migration/p03-playlist-backups.csv).
