# P00.02 routes, aliases and player composition

Evidence: [source-audit.json](source-audit.json), generated with TypeScript 6.0.3 from the pinned archive. It contains all 33 enum declarations, 30 registered enum paths, pathless/index routes, wildcard registration, five remote routes, import counts and per-source hashes. Parsing JSX expressions handles multiline attributes. This is source reachability, not observed navigation.

The renderer entry is `src/renderer/index.html -> main.tsx -> app.tsx -> router/app-router.tsx`. Desktop and full web use the same renderer entry with different Vite configuration. `AppRouter` registers a `HashRouter`, authentication/titlebar/application/responsive wrappers and context modals. `AuthenticationOutlet` shows a loading state; its hook owns authentication redirects. Login/action-required/no-network routes use separate wrapper branches. An authenticated invalid route reaches the wildcard component. Home has both an index and explicit `/` registration; these are one workflow.

| Registered surface | Components and native mapping |
| --- | --- |
| `/`, `/search/:itemType`, `/favorites` | HomeRoute, SearchRoute, FavoritesRoute -> Home, Search and Favorites navigation |
| `/settings`, `/now-playing` | SettingsRoute, NowPlayingRoute -> Settings and Now Playing panes |
| `/library/albums`, `/library/albums/:albumId`, `/library/albums/dummy/:albumId` | AlbumListRoute, AlbumDetailRoute, DummyAlbumDetailRoute -> album list/detail, including reconstructed album detail |
| `/library/artists` and `/:artistId` with discography/songs/top-songs/favorite-songs | ArtistListRoute; shared AlbumArtistDetailRoute, AlbumListRoute, SongListRoute, AlbumArtistDetailTopSongsListRoute, AlbumArtistDetailFavoriteSongsListRoute -> Artist detail tabs |
| `/library/album-artists` and `/:albumArtistId` with the same four child suffixes | AlbumArtistListRoute and shared detail/list components -> separate Album Artist scope and tabs |
| `/library/genres`, `/:genreId`, `/library/songs`, `/library/folders` | GenreListRoute, GenreDetailRoute, SongListRoute, FolderListRoute -> genre/song/folder navigation |
| `/playlists`, `/:playlistId/songs`, `/radio` | PlaylistListRoute, PlaylistDetailSongListRoute, RadioListRoute -> Playlists and Radio |
| `/action-required`, `/login`, `/no-network`, `*` | Recovery/login/invalid components -> explicit recovery and navigation errors |

`EXPLORE`, `PLAYING` and `SERVERS` are declared but unregistered. A whole-source search finds no `AppRoute.EXPLORE`, `.PLAYING` or `.SERVERS` uses. Mark those declarations unused in this router only; do not discard explore/home content, Now Playing or server management, which have active surfaces. No redirects from those three paths are registered. No other declarations are labeled unused without evidence.

Both artist families deliberately compose shared components; they are distinct identity scopes, not duplicate requirements. The two Home registrations are aliases by composition. Modal registrations also preserve import preview, playlist edits, sharing, tags, lyrics, list configuration, shuffle-all, fullscreen and visualizer settings even where they have no standalone route.

The remote entry `src/remote/index.html -> index.tsx -> app.tsx -> router.tsx` has index player, `library`, `playlists`, `radio`, `queue`. Remote controls remain a separate deliverable, mapping to five responsive controller surfaces; they send commands to the desktop state owner rather than owning playback.

`/@/renderer`, `/@/shared`, `/@/i18n` and `/@/remote` resolve to matching `src/` directories. Main/preload use `/@/main` and `/@/preload`. CSS `?inline` is a Vite resource query, not a missing file. The target-dependent `/@/lyrics-conversion-api` resolves to main `furigana.ts` in full web, a development API in desktop serve mode, and Electron IPC in desktop production. The `path` shim is web/development-specific. Exact aliases are in the three hashed Vite configurations; a TypeScript paths table alone does not establish runtime resolution.

The conservative graph scans 1,000 source JS/TS modules and 7,795 import/export/require edges, including type edges. Renderer, remote, main and preload roots reach 862/156/44/34 source modules respectively. No literal source namespace import remains unresolved; the conversion API is reported separately, and no computed import/require call was found by this scanner. Conditional build plugins, asset imports and runtime branches still need target execution; graph membership does not prove a symbol executes.

`src/renderer/features/player/audio-players.tsx` is zero bytes with no incoming scanned imports or root reachability. Its SHA-256 is the empty-file digest. It has no independent behavior to port. The active `components/audio-players.tsx` is 14,996 bytes, imported by `app.tsx:32` and `api/subsonic/subsonic-controller.ts:12` for codec profiles. It composes playback, reporting, remote, timers, radio and capture hooks. Native implementation must break that UI/backend codec coupling at the application/engine boundary. WaveSurfer source remains inventoried; absence from this composition is not sufficient evidence to drop waveform behavior.

Each [journey](journeys.csv) supplies a stable function anchor, concrete actions, `target_expected` acceptance and proposed native surface, with a governing ADR in `decision_refs`. These rows are `source-anchored-target-proposal`, not captured original behavior: `reference_outcome` is `not-captured` and runtime status is `not-run`. For example, F28 obsolete-refill rejection and F42 inert CSS are deliberate target changes under ADR-009/001; do not use them as original pass criteria. One anchor is an entry into source investigation, not proof that one function implements the entire journey. All [migration dispositions](../migration/p00-map.csv) remain `unmapped` until implementation evidence exists.
