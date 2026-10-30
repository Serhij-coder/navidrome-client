# ADR-026: Subsonic song-list branches and local ordering

Status: accepted for bounded P03.03/P03.04 continuation, 2026-10-30. No phase exit or production transport choice.

## Evidence and routing

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` controller getSongList at `1543:18`, sortAndPaginate at `268:1`, getLibraryId at `2504:1`, shared/api/utils.ts sortSongList at `144:29` and normalizeSong at `173:23` define these rules. The controlled replay binds 13 original expressions, five enum bindings, five API routes and pinned Lodash. Use existing SongListQuery/Song/Page with one get_songs adapter operation and two private cohesive implementation modules. No new crate, dependency, persisted schema or domain DTO.

Priority is nonempty search, supplied genre IDs, true favorites, supplied album/artist selection, then general search3. Empty genre vectors still choose getSongsByGenre and omit the genre value. Search returns the requested start index; genre and general paths return zero, although all send the requested wire offset. These server-result paths keep unknown totals and server order, retain all returned rows even when the server ignores size, and do not locally sort or consume RNG. Forward exact search text, optional size and only the first folder/genre. General search uses an empty search string. Remaining year/rating/image filters are ignored as in source.

Favorites normalize the entire raw response, choose albumArtistIds before artistIds (including an empty preferred vector), and filter nonempty selections against normalized albumArtists. Filtered rows are then locally sorted and paginated; limit zero means all remaining rows and the total is the complete filtered count. Album/year/rating filters do not gain inferred behavior. This list filter intentionally differs from the separately preserved raw favorite count.

Selected albums and artists use serial bounded requests instead of Promise.all. Request direct albums, selected artists, then every discovered album in original order. Retain duplicate selectors, duplicate discovered albums and repeated songs. Skip only HTTP failures on artist/album fan-out; other failures return no partial page. An explicitly empty selection returns an empty page without I/O. Return all normalized songs at offset zero with an observed total, ignoring sort/page inputs and album disc-title maps, as the original branch does. Missing/null/scalar required album-song arrays fail rather than fabricating valid songs from flatMap artifacts. Serial processing may stop earlier on errors than the source's concurrent promises.

## Local sorts and RNG

Cover every declared SongListSort variant in both directions. Use stable UTF-16 lowercase text order, nullable/missing key order and source secondary keys. Artist sorting omits the album secondary key. Name uses album as its only secondary key. Album artist uses the first normalized album artist; genre uses the first genre with absent-first values ordered as undefined. Comment/last-played fields remain null and therefore sort by album/disc/track secondary keys. Explicit-status is unhandled upstream and retains order. ID is source order or literal reverse; sortName is case-sensitive. Ratings, favorite flags, BPM, channel counts, play counts, lexical release dates, release years and track years follow normalized source keys. Preserve raw finite floating milliseconds for duration sorting independently of the published checked integer milliseconds.

Forward shuffle consumes one Random draw per row, including the last. The upper 53 bits of each u64 give the source Math.random-compatible fraction. Direction has no effect. Only the favorite random branch invokes this local RNG; provider futures/state need not be Send. No secure entropy or RNG uniformity guarantee is inferred.

## Created-date policy and characterized differences

Recently-added sorting in source parses createdAt, extracts the host-local calendar day and reconstructs Date(year,month,day), including its year 0..99 to 1900..1999 remapping. The target uses a narrow portable ISO parser and UTC calendar days, with offset conversion, ISO day rollover, fractional-ms truncation, 24:00, expanded signed years and JavaScript's date clip range. A pinned original probe orders two songs differently under UTC and Europe/Bratislava; the fixture retains both observations. Target ordering matches UTC. Host-local timezone parity is explicitly not claimed.

Only this local sort validates created timestamps. Non-ISO strings (including source-accepted English/legacy integer forms) and invalid/clipped ISO timestamps fail Malformed instead of using platform Date.parse coercion or Lodash's inconsistent NaN ordering. Other sorts/server-order branches continue to preserve createdAt text under the existing normalizer. Fixtures preserve original outputs and explicitly annotate these target errors. This is an intentional checked-input compatibility difference, not a full Date.parse replacement or proof that arbitrary server date strings are supported. Full host parsing/coercion compatibility remains outside the selected contract and must remain visible in parity accounting.

The parser reads a bounded number of time components without collecting an input-sized vector. Integer Gregorian arithmetic has bounded year inputs and no OS/timezone dependency. Existing ADR-014 metadata/ID/time/optional-field policies remain in force.

## Bounds and remaining work

One aggregate work budget counts operation root, every request, observed artist-album rows, songs and supplied metadata, including rows later excluded or sliced. Validate scoped server, all selector cardinalities, page integers and search length before dispatch. Reuse byte limits, exact correlation and pending/between-request/final context guards, including no-I/O selections. These are logical work bounds; concrete reception limits, wakeups and UI/runtime scheduling remain caller/transport responsibilities. No authenticated profile binding is inferred.

See [contract and verification](../p03/song-lists.md) and [migration map](../migration/p03-song-lists.csv). Folder navigation, top/favorite composition, service metadata, mutations/streams/reporting, concrete transport, live conformance and all phase exits remain open. No queue reducer or playback engine is added.
