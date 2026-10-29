# ADR-019: local playlist and genre filtering, sorting and pagination

Status: accepted for bounded P03.03/P03.04 continuation, 2026-10-29. General server pagination/count probing and phase exits remain open.

## Evidence and implemented operations

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` Subsonic controller getPlaylistList at `1227:22` and getGenreList at `1138:19` fetch complete collections, filter/search and sort locally, normalize all retained rows, then use sortAndPaginate at `268:1`. getPlaylistListCount at `1278:27` independently fetches and filters raw rows without normalizing. Normalizers are normalizePlaylist at `407:27` and normalizeGenre at `440:24`. API declarations and Lodash 4.17.21 filter/orderBy are bound in the optional reference runner. Source BaseQuery requires sortBy/sortOrder (domain-types.ts:276); the existing portable typed list queries preserve that contract.

Use existing PlaylistListQuery/GenreListQuery and Page models, without a new DTO/crate. Implement every declared playlist sort (duration/name/owner/public/songCount/changed) and genre sort (albumCount/name/songCount). Source updatedAt sorting uses raw changed text; dates are not parsed. Name/owner search and sort use lowercase strings. Compare string keys by UTF-16 code-unit order to match JavaScript/Lodash. Stable sorting retains source order for equal keys in both directions. This is case conversion, not locale collation or Unicode normalization. Rust and the replay Node runtime supply their respective Unicode tables; pinned representative Greek/contextual sigma, dotted-I and supplementary characters are characterized, not a universal cross-version Unicode certification.

Lodash sorts ordinary values before null before undefined in ascending order, reversing those groups for descending. Optional-chained owner converts both missing/null to undefined; changed/public/count retain null-versus-missing for sorting even though normalized domain fields can both become None. Metadata normalizes through the existing playlist helper. Genres retain exact names as IDs, optional counts and absent artwork. No new image URL construction is added.

Source pagination is local array slicing. startIndex is retained even beyond the collection length; count is the complete filtered count. Limit missing or zero means the entire filtered length, and a positive limit caps returned rows. Normalize every retained row before slicing, so malformed out-of-page records still fail. Filtering happens before normalization, so excluded malformed metadata need not fail. Count-only reads do not validate unused metadata: unfiltered null rows are counted just as the source counts raw array length. A search requires object/name strings and can therefore fail on the same rows. These distinct operations are not fused or automatically cached.

Playlist excludeSmartPlaylists and genre music-folder selections are ignored by this adapter, matching source. List/count requests carry no pagination/filter parameters to the server. No generic server paging or total-count discovery algorithm is implied.

## Validation, limits and differences

Existing ADR-014/ADR-017 checked ID/count/duration and typed malformed-field rules remain. Consumed genre counts are nonnegative safe integers; missing/null counts are absent. Invalid fractional/string counts or missing required genre names are rejected instead of leaking wire values into domain. Query start/limit use nonnegative safe JS integers. Search text is capped at 8192 UTF-8 bytes before dispatch; over-limit is Capacity. Domain query types prevent negative/fractional paging values. Unknown sort variants are not representable.

All operations validate nonblank <=128-byte server identity and capacity for one root, preserve the caller's explicit server, use exact request/generation correlation and existing byte/cancellation/deadline checks, and check completion context after filtering/sort/normalization/slicing or error. Futures remain local/non-Send. Caller binds base/auth/server to one profile.

Work limits count root plus every raw input row before search or paging; a tiny returned page cannot bypass full-list work limits. Per-array checks also apply. Sorting remains O(n log n) within that bound; string work is additionally bounded by response bytes and the query search cap. These are work/input limits, not exact heap, UI latency, streaming reception or runtime scheduling guarantees. Normalization stays in the adapter; no UI consumer/runtime scheduler is introduced.

## Verification and remaining work

The 57-case fixture binds six original expressions/helpers/controllers, two method/path declarations, four source hashes, the compiler and optional Lodash module hash. It covers all sorts/directions, stable ties, missing/null ordering, case/UTF-16 search, counts, absent/empty bodies, page limits/offsets, filtering before normalization and errors outside a returned page. Default provenance/tampering guards require tracked inputs only. Replay evaluates selected original expressions against in-memory responses, without module initialization, network or profile access.

See [contract and checks](../p03/local-lists.md) and [migration map](../migration/p03-local-lists.csv). Other library lists/counts, offset probing/batching, folder trees, search operations, mutations, production HTTP, live server conformance and phase exits remain open.
