# ADR-030: playlist mutation requests and explicit removal positions

Status: accepted for bounded P03.05 continuation, 2026-10-30. The backup/remove/add replacement workflow and phase exit remain open.

## Evidence and interface

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` controllers addToPlaylist `302:20`, createPlaylist `414:21`, deletePlaylist `468:21`, removeFromPlaylist `2198:25`, setPlaylistSongs `2423:23` and updatePlaylist `2484:21` define these six operations. The source client uses repeated-array serialization. The replay binds six original expressions, two source files and three API routes. Existing ADR-029 acknowledgement/error contracts govern all operations.

Add portable created-playlist, metadata, song-list and position-list models plus PlaylistIndex. The latter is an explicit nonnegative wire position, distinct from PlaylistEntryId, SongId and QueueOccurrenceId. No automatic conversion from old playlist snapshots is provided: the earlier read contract explicitly treats positional entry IDs as snapshot identities rather than durable mutation handles. Caller-side snapshot/revision validation remains an application responsibility. Reuse the existing mutation module and progress owner, adding MutationValue for normalized values returned after acknowledgements. No crate, dependency, schema or concrete transport is added.

## Source wire rules

Create forwards only name and returns checked server-created ID/name with explicit server scope. Other shared create fields are not inferred as supported. The returned canonical name may differ from input. Metadata update forwards required name, optional comment/public and playlistId; empty strings and explicit false remain present. Unconsumed owner/smart/sync fields do not gain capabilities.

Append uses updatePlaylist with songIdToAdd. Direct set uses createPlaylist with playlistId/songId. Both preserve repeated SongIds and order. Empty arrays omit repeated parameters, matching source repeated-array construction; no automatic chunking changes one request into multiple operations. Live empty replacement behavior still requires server conformance and is not inferred from a disposable double. Direct setPlaylistSongs is separate from replacePlaylist, whose source performs backup/remove/add.

Removal forwards songIndexToRemove in exactly the caller's order, including repeated positions. Do not silently sort, deduplicate or substitute song IDs. A disposable workflow removes positions 2 then 0 from repeated song occurrences, preserving the intended remaining song. Validate safe-integer positions without inventing a current playlist length or claiming concurrency protection. Delete forwards the explicit scoped playlist ID. Server-side permissions/errors remain authoritative; no administrator-role assumption or automatic retry is introduced.

## Acknowledgements and failure handling

Progress can now return a validated response body privately before normalizing a created value. It increments the wire acknowledgement count at the same boundary as ADR-029 and retains all earlier operation behavior. A malformed created-playlist ID/name after a valid envelope returns an error with one acknowledged request and no pending unknown exchange. This does not mean no resource was created: the disposable transport demonstrates a created resource with unusable response metadata. Callers must reconcile instead of retrying. An unacknowledged response loss remains outcome_unknown=true. Final context checks cover normalized values, errors and ordinary receipts.

Validate scoped playlist identity, input cardinality, IDs/positions, request/response bytes and exact correlation. All arrays are bounded before constructing repeated pairs; pair/byte caps reject oversized one-request operations before effects. Blank scoped playlist IDs and unusable created IDs fail rather than becoming mutation handles. Existing source literal-null/undefined and stricter-ID policies remain governed by ADR-012/ADR-014. Futures remain local/non-Send. No batch atomicity, rollback, per-entry success or exactly-once effect is claimed.

## Replacement and integration follow-up

The source replacePlaylist at `2212:22` reads songs and detail, writes an IndexedDB backup at playlist-backup-${id}, removes reversed positions and appends new IDs. That key omits server scope and its storage is browser-specific/unversioned. This increment does not implement that destructive workflow without its required backup boundary. The next bounded continuation must provide a versioned, isolated server/playlist-scoped backup, failure/interruption fixtures and durable native/browser integration before removal. It must retain partial/uncertain outcomes and cannot claim transactional replacement. Direct set is not presented as a substitute for the backup workflow.

See [contract and checks](../p03/playlist-mutations.md) and [migration map](../migration/p03-playlist-mutations.csv). Application snapshot validation/cache reconciliation, live disposable-server conformance, replacement backups, remaining mutations/streams/reporting and all phase exits remain open.
