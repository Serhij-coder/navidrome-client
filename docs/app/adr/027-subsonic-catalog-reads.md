# ADR-027: composed song views and catalog service reads

Status: accepted for bounded P03.03 continuation, 2026-10-30. No phase exit or production service activation.

## Evidence and contracts

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` controller getFavoriteSongs at `987:23`, getTopSongs at `2099:18`, getInternetRadioStations at `1180:31` and getScanStatus at `1416:20` define these four reads. FavoriteSongListQuery and TopSongListQuery in domain-types.ts:1381-1386/1554-1560 define the optional type/limit fields. normalizeInternetRadioStation at `497:39`, hasFeature at `46:27` and the existing song-list/normalizer helpers supply composition rules. The replay binds 19 original expressions, eight API routes, six enum bindings and seven source files, including features-types.ts extracted from the immutable archive and matched to the source audit hash. Initial replay rejected the missing feature-enum input; it passed after the verified file was supplied. Builds/CI need only tracked fixtures.

Add typed favorite/rating and community/personal query modes to portable domain models, with source defaults represented by enum defaults. Scoped artist identity is explicit. Add four free adapter functions in one cohesive catalog_reads module; reuse get_songs and existing normalized types. No crate, dependency, schema, concrete HTTP, UI, timers, radio playback or scan mutation is added.

## Composed songs and feature gating

Favorite/rating and personal-top views fetch the complete selected artist's album songs through the existing bounded serial fan-out. Favorites retain userFavorite; rating retains userRating > 2; personal top retains playCount > 0. They sort by the exact source keys/directions, keep stable ties and preserve repeated songs. Favorite descending nullable ratings place unrated favorites first, an observed source rule. Rating mode sorts rating/favorite/play count descending, then album ID and track ascending; favorite mode swaps the first two keys; personal uses play count descending, album ID and track ascending. Album IDs use case-sensitive UTF-16 order. Disc number is not added as an inferred key.

All three composed views ignore limit, including zero, and retain the underlying unfiltered total even when fewer rows remain. This count/list discrepancy is intentional source compatibility, not a newly inferred filtered total. A fixed nonempty artist query cannot invoke local shuffle; a private invariant provider makes accidental RNG use fail rather than fabricating entropy. The source replay confirms zero RNG draws on every composition case.

Community top songs sends artist ID only when TopSongsByArtistId has a nonempty advertised version list. Absent/empty feature versions use artist name; no version-number ordering or backend-family assumption is inferred. Forward optional count including zero; retain server order and every returned row without local truncation. Its total is observed returned length. The caller binds capabilities, query server, base and auth to one profile; the adapter does not establish that association.

## Service metadata and checked differences

Radio stations retain source order, duplicate string IDs, name, inert stream/homepage text and optional cover-art projection. Empty homepage becomes None; image fields are explicitly null when missing, while thumb/uploaded fields remain absent. The existing portable model additionally carries explicit caller server identity, unlike the source normalizer. The read neither opens URLs nor establishes their suitability for playback/navigation; consuming services still own URL validation. Missing/null station groups are empty; malformed shapes fail. Numeric station IDs are rejected because this wire schema requires strings; checked numeric/string cover-art projection retains the normalizer's toString behavior.

Scan status returns required nonnegative safe-integer counts and a required boolean scanning flag, preserving optional raw last-scan text. Required absent/fractional/negative/unsafe counts or invalid flag types fail instead of emitting undefined or inconsistent typed data. Missing and null optional lastScan become None. These explicit typed shape policies continue ADR-014/ADR-016 rather than supplying invented defaults. No scan is started or scheduled.

## Bounds, errors and verification

Selected-song composition inherits the shared request/row/metadata budget and HTTP-only fan-out skips. Community songs and station lists bound root plus all observed rows/metadata; scan reads are fixed-size subject to wire limits. Validate scoped inputs, safe consumed count values and existing byte/correlation/context limits. Failed results do not publish partial rows. Both composed wrappers recheck final context over inner errors as well as successes and sorting; an exposed regression fixed the earlier malformed-error return that bypassed the outer check. Pending/direct/composed completion tests pass after the fix. Futures remain local/non-Send.

See [contract and checks](../p03/catalog-reads.md) and [migration map](../migration/p03-catalog-reads.csv). Folder navigation, remaining library metadata/operations, mutations/streams/reporting, concrete transport, live conformance and phase exits remain open. This increment does not claim a production favorite screen, radio player, scan worker or playback engine.
