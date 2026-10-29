# ADR-021: artist and album-artist lists/counts

Status: accepted for bounded P03.03/P03.04 continuation, 2026-10-29. General server count probing, live conformance and phase exits remain open.

## Source evidence and distinct queries

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` Subsonic controller anchors are getAlbumArtistList at `533:25`, its count at `572:30`, getArtistList at `909:20` and its count at `949:25`. Both flatten getArtists index arrays and normalize via shared normalizeAlbumArtist at `271:30`. Sorting uses shared api/utils.ts sortAlbumArtistList at `430:36`; local slicing uses controller sortAndPaginate at `268:1`, and getLibraryId at `2504:1` forwards only the first selected folder. The fixture binds original API routes and all helpers/controllers.

Use existing AlbumArtistListQuery/ArtistListQuery, AlbumArtist, Artist wrapper and Page models. No new domain schema, dependency, facade or runtime owner. Album artists return normalized AlbumArtist; artists use the existing distinct Artist wrapper around normalized metadata. The source artist endpoint emits the albumArtist item type because it reuses that normalizer; source replay compares the wrapper's inner fields rather than inventing an upstream artist item type.

Both preserve index/artist order and repeated IDs. Missing/null artist roots/index arrays become empty. Each index's artist array is required. Only the first folder selection is forwarded; empty selection omits the parameter. Source array/string query variants map to the existing portable folder vector. Server identity remains explicit on every normalized result.

Album-artist favorite filtering runs only for true; false means no filter. Artist favorite filtering is ignored by this controller. A nonempty artist role filter runs before normalization: absent/null roles pass, empty arrays do not pass, and arrays pass on exact membership. Empty role means no filter. Search and album-artist favorite filtering happen after normalization, so malformed metadata can fail even when the later search would exclude it. Unconsumed embedded album data and roles without an active role filter are ignored, matching source.

Similar artists from the list normalizer are retained, with absent/null arrays becoming present empty. This differs from detail's explicit forced null and info's absent/empty distinction. Nullish artwork fallback retains empty covers; numeric covers stringify under ADR-020. Related rating zero becomes null for list normalization, whereas get_album_artist_info retains zero. Shared private helpers expose these endpoint-specific policies explicitly; existing detail/info regressions remain unchanged.

## Sorting and count semantics

Name sort uses lowercase UTF-16 comparison and stable ties. Album-count sort uses the normalized numeric count, followed by name ascending even when the primary direction is descending. All other declared variants retain source order for this backend. Rating is always null in normalized album artists, and the source favorite sorter reads starred even though normalization emits starredAt/userFavorite. Its undefined sort keys therefore retain source order. Random and the other unsupported sort variants use the default unchanged branch. These observed ineffective sorts are characterized, not silently repaired or represented as effective capabilities. No RNG call or artificial favorite ranking is added.

Local pagination retains source zero-limit behavior and total-before-slicing. Count operations delegate to the same complete normalization/filter/sort/page flow with startIndex reset to zero, then return total_record_count. They retain the limit but return the full filtered count. They do not bypass normalization like playlist raw counts do. The count and artist-wrapper projection paths also recheck completion context.

## Validation and bounds

Existing ADR-014/ADR-016/ADR-020 numeric, required-field, timestamp, ID and related-artist policies remain. Malformed role strings/array entries are rejected when role filtering consumes them; the source string includes operation can accept a malformed string, explicitly characterized as a target rejection. Missing/null similar IDs, invalid album-count coercions and malformed required names remain typed errors.

Every read checks scoped nonblank <=128-byte server identity, safe page numbers, search/role text caps of 8192 UTF-8 bytes, folder-selection cardinality, request/response limits, exact correlation and final cancellation/deadline priority. Futures remain local/non-Send. Root plus every index/raw artist row consumes aggregate work before exclusion; consumed role rows and retained similar-artist rows also count, including duplicates. Per-array limits remain. Bounded work does not certify reception allocation, UI latency or runtime scheduling, which stay outside this adapter. Caller binds base/auth/server and retains server-scoped context for related metadata.

## Evidence and remaining work

The 83-case fixture replays eight original expressions/controllers, five original source hashes and one getArtists method/path declaration with hash-verified TypeScript and Lodash. It covers every declared sort/direction, stable order, favorite/role/search timing, first-folder behavior, absent/empty arrays, retained similar metadata, paging/counts and malformed observations. Default provenance/tampering guards use tracked inputs only.

See [contract and checks](../p03/artist-lists.md) and [migration map](../migration/p03-artist-lists.csv). Album/song list algorithms/count probing, folder trees, search operations, top/favorite song composition, production HTTP, live server behavior and all phase exits remain open.
