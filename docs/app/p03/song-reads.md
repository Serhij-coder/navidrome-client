# Bounded Subsonic song detail read

P03.03 now has one implemented read: `feishin_subsonic::get_song`. It sends `GET getSong.view?id=...` through the existing injected `ProtocolTransport`, requires HTTP 200 and a successful Subsonic envelope, and returns the portable domain `Song`. No HTTP runtime, dependency, domain change, retry, cache, login or broader backend facade is added. See [ADR-014](../adr/014-subsonic-song-reads.md), [migration map](../migration/p03-song-reads.csv) and [verification](song-reads-verification.md).

The caller supplies `Scoped<SongId>`, validated base/authentication, `Clock` and `OperationContext`. The caller must select base, credentials and scoped server from the same profile: the existing request context has request/generation/deadline but no server identity. Returned songs and genres retain the explicit scoped server, including when two servers share a song ID. Server identity must be nonblank after whitespace trimming and at most 128 UTF-8 bytes, matching the persisted P02 profile boundary; its original bytes are retained. Empty requested song IDs are malformed, and a returned song ID different from the requested ID is stale. The 128-byte profile limit does not apply to source song IDs, which retain the existing bounded query-construction limit. This detects mismatched results but cannot authenticate a server/profile association.

The existing protocol exchange enforces request/response byte limits, cancellation, inclusive deadlines, exact response correlation, HTTP and protocol failures. The song operation rechecks context before planning and after normalization, including normalization and returned-ID errors. Cancellation/deadline takes priority over those results. The operation requires capacity for one song, bounds each supplied artist/album-artist/contributor/genre collection and the resulting artist list after remixer insertion. Item limits are per collection/output, not a summed heap budget. Response limits precede JSON parsing but follow transport allocation; production transports must bound streaming reception. Pending futures remain local and may capture non-Send state. Transports must arrange wakeups; this wrapper supplies no timer or forced socket abortion.

## Normalized fields

The exact pinned source is Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8`; function anchors are in the migration map. All fields emitted by the detail normalizer are compared against [18 synthetic source replay cases (15 normalized results and three source failures)](../../../tests/fixtures/p03/subsonic-song-reads.json).

| Fields | Source behavior retained |
| --- | --- |
| Identity, name, sort name, server/type | Stringify string/numeric IDs; title supplies both names; attach selected server and Subsonic type |
| Album, album ID, artist name, path | Missing/null strings and IDs become empty strings; album/path use `Some("")` in domain |
| Album artists/name | Supplied array (even empty) suppresses list fallback; absent array yields one legacy artist; nonempty array joins names, otherwise legacy artist name |
| Artists/participants | Preserve contributor role/subrole grouping; present contributors, including empty array, suppress legacy artist fallback; append remixers absent from original artist IDs, retaining repeated remixers |
| Artist and genre artwork/user data | Related artists have absent artwork/rating and false favorite; genres use selected server, name as ID, absent counts/artwork |
| Genres | Supplied array, including empty, suppresses legacy genre; otherwise nonempty legacy genre becomes one item |
| Bit depth, BPM, channels, sample rate, user rating | Zero becomes absent; other validated modeled numbers retained |
| Bit rate, play count, size | Missing bit rate/play count become zero; size is required |
| Disc and track numbers | Missing/zero becomes one |
| Duration | Seconds multiplied by 1000; checked integer milliseconds, with truncation described in ADR-014 |
| Container | For `audio/`, use the first component after the slash, including an empty component; other content types yield absent |
| Created/updated | Preserve raw created string without timestamp parsing; updated is empty |
| Track/original date and year | Nonzero numeric year becomes string date and year; zero/missing/null becomes absent |
| Release date/year | Complete numeric year/month/day uses lexical `YYYY-MM-DD` check, retaining impossible calendar dates; invalid complete shape yields absent date and year zero; incomplete date falls back to track year |
| Replay gain/peak | Emit pair only if either corresponding component is nonzero; retain zero component when the other is nonzero; missing components become domain absent |
| Cover art, MusicBrainz recording ID | Stringify cover art and discard empty value; discard empty recording ID; no artwork URL construction |
| Explicit/favorite | Map exact `explicit`/`clean`; other values absent. Favorite follows boolean or nonempty timestamp/string truthiness |

The source detail call supplies no playlist index or disc-title map: playlist item ID and disc subtitle are absent. Codec, comment, compilation, blur/thumb hashes, folder ID, last played, library ID/name, lyrics, MusicBrainz album/type/release-group/track IDs, missing flag, tags and track subtitle are always absent, even if wire fields with those names exist. Source `timestamp` and `_custom` are ignored. Domain-only resumable/resume-position values remain absent. No raw DTO or authenticated stream/image URL enters domain output.

## Scope still open

Only this detail endpoint is implemented. Remaining P03.03 reads, all P03.04 pagination/count/batching, mutations, stream/download/image URL operations, lyrics, reporting, jukebox, production transport, application adoption, server conformance and phase exits remain deferred. This work does not establish live server, audio/device, browser runtime or AppImage evidence.
