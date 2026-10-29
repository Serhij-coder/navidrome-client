# ADR-018: bounded folder, role and recommendation reads

Status: accepted for bounded P03.03 continuation, 2026-10-29. Phase gates and production transport remain open.

## Evidence and portable contracts

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` Subsonic controller anchors are getMusicFolderList at `1193:25`, getRoles at `1390:15`, getRandomSongList at `1362:24` and getSimilarSongs at `1502:22`. getLibraryId at `2504:1` returns only the first selected folder. The controlled replay also binds the original API route declarations and shared song normalizer/helpers. In particular getSimilarSongs uses an extensionless path; the other routes use .view. This is retained explicitly rather than inferred from endpoint naming.

Use existing MusicFolder, Song and Page models. Add portable RandomSongQuery and SimilarSongsQuery from the source query types (domain-types.ts:1442 and :1971); replace the source role string/object union with ArtistRoleChoice::AllArtists or Role(String). AllArtists is semantic and unlocalized; the consuming UI will supply its localized label. Empty named roles remain distinct from AllArtists. This changes representation, not source filter choices.

Folder lists require the musicFolders.musicFolder array, preserve order/duplicates and stringify supported string/numeric IDs. Missing required containers fail, as the original controller throws. Roles scan artists.index[].artist[].roles, deduplicate exact strings, sort by JavaScript UTF-16 code-unit order and always prepend AllArtists. Missing/null artist root/index and role arrays follow source empty fallbacks. Missing artist arrays remain malformed. A source malformed string roles value is iterable and yields characters; the target rejects it as incompatible with a typed string-array schema.

Random songs forward optional fromYear, genre, first musicFolderId, size and toYear. Empty folder selections omit the parameter; further selections are ignored. Zero counts/years and empty string parameters remain present. Source played filtering is ignored by this backend; the typed query retains the user's choice for other backends without pretending it is effective here. Similar songs forward optional count and required song ID, ignoring music-folder selection as the source does. Both keep duplicates and returned order; there is no local truncation to the requested size/count. Missing/null result containers/arrays yield empty results. Random pages use start_index zero and observed returned length.

Similar-song exclusion happens before normalization with strict raw string identity. A numeric ID 42 is retained when requested song ID is string 42, even though normalization subsequently gives it the same song ID. A matching raw string ID is discarded without validating unrelated song fields. This observed source behavior is characterized explicitly; it is not silently corrected. Null song objects still fail. These results contain library identities, not queue occurrence or playlist-entry identities.

## Validation and work limits

Existing safe numeric IDs, checked milliseconds and malformed-value policies from ADR-014 remain. Request counts must be nonnegative safe JS integers; year query fields use the existing portable i32 representation. Required folder names cannot be omitted. Invalid consumed fields produce fixed malformed categories rather than raw DTOs/errors. No network retries, client-side count estimates, capability inference, cache or backend facade is introduced.

All operations check nonblank <=128-byte server identity, capacity for one root, request/response correlation and bytes, inclusive deadlines and cancellation through the local transport boundary. Similar-song identity also requires a nonempty requested ID. Final context checks override normalized successes and errors. Base/auth/server association remains caller-owned. Futures may capture non-Send browser state.

Aggregate normalization work includes one root, every folder/index/artist/song row and each supplied role or retained-song artist/album-artist/contributor/genre row. Duplicate roles and excluded song rows still consume their input work. Skipped song metadata is not traversed. Random folder selection length is checked before planning; only its first value is sent. The synthetic role choice is included in output capacity. Per-array/output checks remain in place. These are work limits rather than exact heap bounds; production transports must bound reception and supply wakeups/timers.

## Verification and remaining work

The [tracked fixture](../../../tests/fixtures/p03/subsonic-collection-reads.json) contains 28 synthetic cases, 14 hash-bound original expressions/controllers and four source-bound method/path declarations. All normalized fields and request parameters are compared. Default provenance/tampering guards need only tracked inputs; optional replay requires explicit hash-verified source/compiler paths and performs no network, original module startup or profile access.

See [collection contract and verification](../p03/collection-reads.md) and [migration map](../migration/p03-collection-reads.csv). General song/album/artist/genre/playlist lists and count probing, folder trees, radios, top/personal songs, artist/album radio, search, live conformance, production HTTP and phase exits remain open.
