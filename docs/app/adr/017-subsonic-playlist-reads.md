# ADR-017: playlist metadata and ordered snapshot entries

Status: accepted for bounded P03.03 continuation, 2026-10-29. No phase exit or production transport claim.

## Source and boundary

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` `subsonic-controller.ts:1212:24` fetches getPlaylist and normalizes metadata. `:1302:26` fetches the same endpoint but normalizes only entries, with each zero-based index passed to normalizeSong. `:1297:25` delegates to that song list and projects song IDs. `subsonic-normalize.ts:407:27` defines metadata; `:173:23` defines songs and optional index identity. The declared Playlist and response types are at domain-types.ts:358, :1313 and :1331; the diagnostic wire schema at subsonic-types.ts:605-617 is stricter than some successful controller outputs.

Use existing portable Playlist, PlaylistSong, Page and distinct IDs. Metadata and ordered entries remain separate reads and do not require each other's consumed fields. Missing/null entry arrays produce an empty page. All entries are fetched, with start_index zero and total_record_count equal to actual array length, independently of metadata songCount. Pagination/filter parameters are not accepted by this low-level operation because the source ignores them here. No cache, mutation, retry, HTTP implementation or facade is added.

PlaylistEntryId is the decimal snapshot position, also retained in Song.playlist_item_id. Repeated song IDs remain distinct entries in order. These IDs are scoped by the requested server/playlist and returned snapshot; they are not durable backend identities across edits or refresh. A later mutation owner must reconcile the snapshot before using positions. Queue occurrence IDs remain a separate identity.

Metadata copies name and owner (including empty strings); owner_id uses the same owner text. Empty comment becomes absent. Cover art stringifies supported IDs and appends raw `&_=` plus nonempty changed text only when cover art exists. This is an opaque image identifier, not a constructed URL. Image URL, hashes, evaluatedAt, genres and size retain source null/empty values. Rules and sync are omitted; target-only uploaded_image is absent.

## Deliberate differences

The checked numeric/ID/unit policy from ADR-014 applies: nonnegative safe integer counts/IDs, finite nonnegative duration and checked integer milliseconds (truncate sub-millisecond remainder). Metadata requires name and duration rather than publishing undefined names or NaN/coerced durations. Owner/public/count absent or null become domain None, preserving the existing nullable domain representation. Supplied owner/comment/changed must be strings and public must be boolean; malformed values are rejected instead of coercing or leaking wire values. Unconsumed metadata on the entry-only read is ignored, matching its controller.

All three operations validate nonblank <=128-byte server identity and nonempty requested playlist ID before dispatch. Returned root playlist ID is required and must match, including for entry-only reads where the source never inspects it. Missing ID is malformed and mismatch is stale. The fixtures explicitly characterize source success without parent ID; the target rejects it to enforce scoped result identity. This guard cannot authenticate a caller's base/auth/profile association, which remains caller-owned.

Entry reads count one root, each entry and each supplied artist/album-artist/contributor/genre row against an aggregate work budget, plus existing per-array/output checks. Metadata reads consume one root and ignore entry content. The album work-budget helper moved unchanged into private read_support. These limits do not count exact allocations or bound transport reception; existing response byte checks follow transport allocation. Native/browser transports must bound reception and arrange cancellation/deadline wakeups. All futures remain local without Send requirements, and final context checks override normalized results and errors, including after ID projection.

## Evidence and remaining work

Twenty-five synthetic cases replay 13 original hash-bound expressions including all three controllers. There are 22 source successes and three TypeErrors. Twelve cases are target rejections (including those three failures), covering malformed source successes and the stricter parent-ID guard. Rust tests compare every emitted field and exercise duplicates, scopes, budgets, invalid input, wire/HTTP/protocol failures, stale correlation and pending/final cancellation/deadlines. Default provenance/tampering checks need only tracked inputs.

See [contract](../p03/playlist-reads.md), [verification](../p03/playlist-reads-verification.md) and [migration map](../migration/p03-playlist-reads.csv). Playlist listing/filter/sort/count, mutations, live conformance, transport integration and all phase exits remain open.
