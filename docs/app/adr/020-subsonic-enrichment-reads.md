# ADR-020: artist enrichment and album/artist radio reads

Status: accepted for bounded P03.03 continuation, 2026-10-29. This is adapter behavior, not a radio/playback engine or phase exit.

## Source and behavior

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` Subsonic controller getAlbumArtistInfo at `504:25` fetches getArtistInfo2, returns null on non-200 HTTP results and otherwise projects optional biography and similar artists. getAlbumRadio at `887:20` fetches getSimilarSongs; getArtistRadio at `954:21` fetches getSimilarSongs2. API declarations bind exact paths, including the extensionless radio endpoints.

Use existing AlbumArtistInfo, RelatedArtist and Song domain types, with distinct scoped AlbumId/ArtistId inputs and optional safe-integer limits. No domain model, dependency, facade or runtime is added. Radio reads preserve all returned songs in order, including repeats and IDs matching the requested source. Unlike get_similar_songs, they do not exclude a matching raw string ID. Missing/null result containers/arrays yield empty results, and returned count is not locally truncated.

Artist info always projects explicit null or string biography; missing/null/empty biography is null. Similar artists preserve missing/null as None and explicit empty as Some(empty). Similar IDs string-normalize, cover art uses a nullish fallback to the artist ID and therefore retains explicit empty cover art. User rating preserves zero through nullish defaulting, unlike song/album rating normalization. Favorite follows the established boolean/timestamp-string policy. Image URL is absent in the info wrapper and null on related artists. Source biography text remains inert data; no markup renderer or external fetching is introduced.

## Compatibility choices and validation

Only artist info maps HTTP errors to an absent enrichment value, matching its explicit source controller branch. Network, protocol, malformed, stale, capacity, cancellation and deadline failures remain typed errors. Final context checks apply after both successful normalization and HTTP fallback. This is a documented compatibility exception to normal error propagation; it does not establish the original HTTP client's behavior on every live failure path. The controlled fixture demonstrates the original controller branch with injected responses.

Missing/null similar-artist IDs become string undefined/null in the source String conversion; the target rejects them rather than inventing usable identities. Missing names and non-string biography are also malformed instead of escaping required domain types. A numeric coverArt is stringified under the existing safe-ID policy, whereas the source's nullish image field can remain numeric despite its declared string type. Fixtures retain these original observations; target comparison projects only that representation correction. Empty covers, zero ratings, duplicates and optional arrays remain intact. Existing ADR-014 checked song/ID/time policies and malformed-field categories apply.

Every operation validates nonblank <=128-byte server identity and nonempty album/artist identity before dispatch, requires capacity for a root and uses existing request/response bytes, exact correlation, inclusive deadlines and cancellation. Info responses have no returned root ID to compare. Base/auth/server association and server-scoped enrichment cache context remain caller-owned; related-artist values themselves contain no server field. No cache integration or authentication policy is inferred.

Radio normalization reuses existing collection song helpers and aggregate root/input-row/metadata work limits. Info counts root plus every similar-artist row, including duplicates. Arrays also retain per-collection bounds. Response reception allocation and wakeups/timers remain transport responsibilities; local futures need not be Send. Work is bounded without claiming UI latency or exact heap limits.

## Evidence and remaining scope

The 26-case fixture binds 12 original expressions/controllers, five source hashes and three method/path declarations. It covers missing/null/empty metadata, zero ratings, cover fallback, malformed source successes, radio duplicates, HTTP behavior and errors. Four Rust test groups additionally cover scopes, input/work/byte bounds, protocol/network/stale failures, pending futures and final context priority, including HTTP fallback. No installer, original module startup, network or real profile is accessed by replay.

See [contract and checks](../p03/enrichment-reads.md) and [migration map](../migration/p03-enrichment-reads.csv). Artist/album lists/counts, top/favorite/personal songs, folder trees, search, radio station CRUD, production HTTP, live conformance and all phase exits remain open.
