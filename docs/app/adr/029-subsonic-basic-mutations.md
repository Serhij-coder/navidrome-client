# ADR-029: acknowledged mutation prefixes and checked user metadata

Status: accepted for bounded P03.03/P03.05 continuation, 2026-10-30. No production mutation adoption or phase exit.

## Evidence and interfaces

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` controllers createFavorite `374:21`, deleteFavorite `430:21`, setRating `2439:16`, startLibraryScan `2455:23`, refreshItems `2187:19` and getUserInfo `2149:18` define these operations. Domain FavoriteQuery at 1077-1081 and RatingQuery at 1122-1126 supply ID arrays/type/rating. The client at subsonic-api.ts:389-391 uses repeated-array serialization; original-controller fixtures retain the selected query arrays and request order. Six original expressions, one LibraryItem enum and five routes are hash-bound in the replay. No upstream installer or live mutation is run.

Add portable typed favorite target families, explicit library-entity rating IDs and query models. Playlist/queue song contexts carry SongId, never PlaylistEntryId or QueueOccurrenceId. Favorites support the source album/album-artist/artist/song/playlist-song families. Other item families do not gain inferred favorite support. Rating retains the source controller's type-independent wire ID behavior across all declared library families. Add one adapter mutations module with receipts/failures and five operations; reuse the existing User model for user info. No crate, dependency, persisted schema, runtime or UI is added.

## Ordered effects and deliberate error policy

Favorite creation/removal sends one star/unstar request with repeated IDs in source order, retaining duplicates and using albumId/artistId/id as appropriate. Empty or blank favorite selections are rejected before dispatch rather than sending an argument-free mutation. Existing request pair/byte caps apply; no implicit chunking changes one batch into several operations. A batch acknowledgement counts as one request, not one confirmation per item.

Ratings preflight every ID, finite score and constructed request/byte limit before the first effect. Preserve source order and repeated IDs, checking context during preflight and before dispatch. Do not invent a score clamp/range: finite values, including fractional values and zero, are forwarded for backend validation; negative zero serializes as zero. Empty rating selections complete without I/O. Build one authenticated request at a time rather than retaining one secret URL per item. Invalid later IDs or request limits cause no predictable partial updates.

The original setRating awaits each request but ignores returned HTTP status. A fixture records HTTP 403 on the second ID followed by a third request and a successful null return; another records a failed protocol envelope with the same outcome. The target intentionally rejects these failures, stops at the failed request and reports the acknowledged prefix. This corrects silent mutation success under the canonical error-handling policy. The source observations remain intact with explicit target error/request-count annotations.

MutationReceipt counts validated HTTP/protocol acknowledgements. MutationFailure carries acknowledged_requests, a conservative outcome_unknown flag and the existing closed ExchangeError. A failure after an exchange is attempted may have applied even without an acknowledgement; the flag may also be true when a later internal guard prevented actual dispatch. A final context error after all acknowledgements retains the known count with no pending unknown request. No automatic retry, rollback, atomic batch, per-ID success or exactly-once effect is claimed. A disposable stateful transport proves that an unacknowledged update can apply before a simulated connection loss. Failed and successful final outcomes recheck context; errors do not erase an earlier prefix.

startLibraryScan and refreshItems both invoke startScan, as source does. Refresh is a global scan trigger, not an item-specific refresh. Permission/auth/protocol failures remain typed; no inferred administrator role bypass or background scheduling is added.

## User metadata and the reserved username boundary

getUserInfo forwards target username to getUser, retains the server's canonical username and returns its ID/name/admin projection with explicit server scope. Unconsumed user metadata stays unknown. Absent/null adminRole becomes false; only actual booleans are accepted otherwise. A fixture records source Boolean("false") producing true; the target rejects that malformed role shape instead of interpreting it as administrator access. Missing usernames fail rather than inventing identity. This is read-only metadata, not authentication, credential retention or profile mutation.

The generic ADR-012 pair planner reserves username as a credential alias. The new integration test exposed that it cannot plan the legitimate getUser target field. Add a crate-private, fixed getUser planner that inserts exactly one encoded username data pair after ordinary protected authentication parameters. It cannot select another endpoint or override u/p/s/t. The generic pair API still rejects username and all other protected aliases. Regression tests verify escaped target values, unchanged authenticated principal, reserved generic rejection and canonical returned names. This is a bounded amendment to ADR-012, not a generic reserved-key exception. Request size is shared crate-privately for mutation preflight without exposing authenticated payloads.

## Verification and open work

See [contract and checks](../p03/basic-mutations.md) and [migration map](../migration/p03-basic-mutations.csv). Source fixtures cover exact array/sequence parameters, all supported target families, source error swallowing, empty selections, role coercion and server acknowledgements. Owned state and context fixtures cover preflight/no-effect failures, partial unknown outcomes, no retries, stale/byte/permission failures and pending/between-request/final guards. All futures remain local/non-Send. Error/receipt Debug contains closed categories/counts, not IDs, credentials or URLs.

Application cache invalidation/reconciliation, uncertain-outcome UI, generic/extension-enabled disposable-server conformance, concrete GET/form transport, playlist/radio/share/queue mutations and the rest of P03 remain open. This does not establish any live write, working client or phase exit. Linux/WASM only; Windows/macOS build/tests remain excluded.
