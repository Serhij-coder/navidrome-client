# ADR-032: bounded native backup worker and local browser port

Status: accepted for bounded P03.05 continuation, 2026-10-30. Destructive replacement orchestration remains the next task.

## Decision and evidence

The source replacePlaylist at src/renderer/api/subsonic/subsonic-controller.ts:2212:22 awaits the backup write before removal. ADR-031 supplies the isolated repository and exact three-case backup-statement characterization. Add PlaylistBackupStore, normalized PlaylistBackup, BackupReceipt, typed BackupFailure and UnixMilliseconds to portable domain; no wire or storage DTO enters domain. Unix timestamps and monotonic deadlines cannot be substituted by type. Reuse feishin-storage without a dependency or crate addition.

NativePlaylistBackups owns a dedicated filesystem thread with exactly one admitted request, including abandoned in-flight work. An exclusive mutable save future serializes the caller, while a shared admission flag protects against repeated future-drop/submission building a detached queue. The factory and non-Send store are constructed/used only on that thread. Completion holds one result and registered waker under a short mutex; no lock crosses an await or filesystem call. Panic cleanup releases admission and publishes a conservative terminal error. Drop requests shutdown and detaches, never joining an uninterruptible OS operation; try_join waits only after the thread finishes.

Validate scope, timestamp, fields, item count and raw payload lower bound before queuing. The worker enforces the actual escaped full-document output budget (the smaller of the context request-byte budget and 256 KiB). Existing recovery reads retain their repository 256 KiB cap; response_bytes is a transport limit, not a request to truncate recovery state. The single owner loads the latest repository revision and saves with that expected revision; this is not cross-process/tab CAS.

Check cancellation and latest start before factory/storage work and again after the preliminary read, then immediately before atomic replacement after the final read/encoding. The worker translates remaining monotonic budget into a native Instant latest-start limit. Every pending future poll checks the supplied monotonic clock/cancellation; caller scheduling must wake on those changes. Completion itself wakes the caller. No runtime timer service or hard filesystem interruption is claimed. Drop cancels not-yet-started work; an already started write may finish. Failures expose may_have_written, including conservative true for pending cancellation/deadline, a panicked provider or native Io. Any failed/late capture must stop destructive server work and be reconciled, regardless of this flag. Successful receipts echo exact request/generation/deadline context and the persisted revision.

BrowserPlaylistBackups implements the same local future using synchronous localStorage and current repository revision. It accepts non-Send browser state and is not a Web Worker/background storage implementation. Both variants preserve other server scopes, repeated IDs and read-only corruption protection. No automatic backup retry, eviction, backend mutation, rollback or application adoption is included.

## Alternatives and consequences

Running NativeStore on the async caller blocks the UI. A general worker pool adds concurrent access to one recovery value without a useful feature requirement. Reusing the existing profile/credential command workers would couple unrelated state owners and schemas. A one-slot dedicated owner is bounded and sufficient for replacement. Additional timer integration, multi-owner coordination and a general persistence service remain explicit integration work.

See [contract/checks](../p03/backup-executor.md) and [migration map](../migration/p03-backup-executor.csv).
