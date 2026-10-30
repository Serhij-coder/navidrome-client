# ADR-025: combined Subsonic search with independent windows

Status: accepted for bounded P03.03/P03.04 continuation, 2026-10-30. No phase exit or production transport choice.

## Evidence and decision

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` shared/types/domain-types.ts:1499-1514 defines optional independent album-artist, album and song windows, folder selection and search text, returning three normalized arrays. Subsonic controller search at `2391:13` forwards those windows to search3, uses getLibraryId at `2504:1` and normalizes every returned row. The source normalizers are normalizeAlbumArtist at `271:30`, normalizeAlbum at `334:24` and normalizeSong at `173:23`, with shared partial-date/metadata helpers. The tracked replay binds 14 original expressions, four literal bindings, five source files and the API route.

Add portable SearchQuery/SearchResults models and one free async adapter search operation. Keep DTOs, request construction and normalization in feishin-subsonic, reusing the existing album/artist/song implementations. No crate, dependency, schema, HTTP runtime, general Backend facade or cache integration is added. Server identity is explicit on the query and all normalized values; caller binds it to base/auth. No queue/playlist occurrence identities are invented for repeated search songs.

Forward optional windows independently, including explicit zero and safe-integer maximum values. Omitted values stay omitted; absent search text is not replaced with an empty query. Preserve exact supplied text and only the first selected music folder. Empty folder selection omits the parameter. Retain server order and every returned row in each family, including duplicates and results larger than a requested zero/small limit. Do not apply local filtering, truncation, sorting, total-count headers or inferred totals. Missing/null result parents/groups become empty arrays; malformed shapes remain typed failures.

## Bounds and compatibility

Validate all six numeric window inputs and folder cardinality before dispatch, along with the existing server/search/request limits. Use one aggregate work budget across artist rows, supplied similar-artist metadata, albums, nested album songs/metadata and root songs/metadata. Tiny or zero limits cannot bypass normalization work. The budget bounds logical work rather than exact heap allocation or streaming reception. Exact correlation, byte limits and local pending/final cancellation/deadline guards apply. A malformed later family returns no partial result; final context checks override successful and malformed normalization. Local futures and transport doubles need not be Send.

Inherit checked ID/count/metadata/time and representation decisions from ADR-014, ADR-016, ADR-020 and ADR-021. Fixtures retain source floating duration, omitted fields and numeric similar-artist cover-art observations while comparisons explicitly project integer milliseconds, typed optional fields and string IDs. Missing album title uses the existing name fallback. Source JavaScript accepts a string releaseDate by falling back to year; the checked normalizer requires the supplied non-null field to be an object and rejects it. This inherited difference is now characterized separately from a valid rich mixed-result case. Invalid result-parent scalars that JavaScript optional property access treats as empty are also rejected. No broader JavaScript coercion or server-runtime parity is claimed.

Alternatives - three unrelated search calls, one shared window, local pagination or inferring a total - change source requests or outputs and are not selected. Reusing standalone collection helpers with fresh per-family budgets would fail to bound the combined operation, so this operation owns the shared normalization budget.

See [contract and checks](../p03/search.md) and [migration map](../migration/p03-search.csv). Song-list sorting, folder navigation, top/favorite composition, remaining mutations/streams/reporting, concrete transport, live conformance and all phase exits remain open.
