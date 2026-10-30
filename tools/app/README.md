# Application tooling

Keep maintained application build, test, benchmark, packaging and development tools here. P00 adds `check-p00.py` (tracked evidence/link validation), `p00-library.py` (bounded synthetic wire pages), and optional `p00-source-audit.cjs` / `p00-wire-check.cjs` reference harnesses. Prerequisites, limits and exact commands are in the [baseline guide](../../docs/app/baseline/README.md), [fixture guide](../../tests/fixtures/p00/README.md) and [run record](../../docs/app/baseline/reference-run.md). None is a Rust application or proof of runtime parity.

Put application tests and fixtures, including tests for these tools, in the dedicated [tests directory](../../tests/README.md). Tools here may run that suite but must not become a separate location for application test code.

`p00-reference-runtime.cjs` loads verified pinned source with explicit headless boundaries for scenarios under `tests/reference/`. The scenario `tests/reference/p00-live-capture.py` probes only the six pre-provisioned disposable loopback fixtures, redacts output, and mutates only its own temporary playlists. Read the [headless guide](../../docs/app/baseline/headless-characterization.md) and [server runbook](../../docs/app/baseline/live-servers.md) before using these optional tools; neither belongs in application build/CI dependencies.

`p00-adapter-runtime.cjs` loads verified original controllers, clients and normalizers with frozen Axios and an offline transport adapter. [Replay scenarios](../../tests/reference/p00-adapter-traces.cjs) live under `tests/reference/`, with exact saved results and default artifact checks under `tests/`. Read the [adapter guide](../../docs/app/baseline/adapter-characterization.md) for commands and boundaries; no network request is sent by this harness.

Tools required by the application or its CI must use tracked inputs and work without the ignored rewrite inventory, archived source or analysis scripts. Explicitly optional reference harnesses accept external reference/parser/dependency paths and are excluded from application builds/CI. Default evidence checks and the generator use Python standard library and tracked inputs only. Existing scripts directly under `tools/` remain local rewrite-analysis references. Maintain app tools in the same task as the behavior or release process they support.

Optional original-UI scenarios under `tests/reference/` use external Playwright/Electron and controlled disposable profiles/services; see the [browser/desktop/remote guide](../../docs/app/baseline/browser-characterization.md). The generic browser environment is independent infrastructure and is not required by default artifact tests, application builds or CI. Saved screenshot/DOM/source hashes and selected observed flows are checked with tracked inputs only.

P01 adds `build-web.py`, `package-appimage.py`, `notices.py`, a browser shell and original development icon. Build prerequisites, exact commands, checksum policy, privacy and development-only packaging limits are in the [P01 guide](../../docs/app/p01/README.md); dependency text provenance is in the [license guide](../../docs/app/licenses/README.md). Their tests are under `tests/tooling/`; no ignored plan, archive or analysis script is a build input.

P01 continuation adds `test-native.py` (normal Cargo test execution with payload-free failed-test CI annotations) and `probe-native-media.py` (explicit offline FFmpeg filter/waveform evaluation on generated local tones). See [commands and limits](../../docs/app/p01/continuation.md). Optional native/AT-SPI/browser test scenarios remain under top-level `tests/`; no ignored reference is an application build prerequisite.

`python3 tools/app/test-native.py --native-controls` runs the three exact real mpv/controller, handoff and private-bus MPRIS probes sequentially. It needs mpv and dbus-daemon, uses owned tones/null output and stops at the first failure. This mode and `--external-audio` are mutually exclusive; neither runs by default. See the [hosted failure and diagnostic record](../../docs/app/p01/native-probe-ci.md).

`--gstreamer-probe` checks the optional Linux GStreamer library/candidate fixtures; `--gstreamer-codecs` invokes its ignored external mpv/FFmpeg comparison. They require GStreamer development/runtime packages and are mutually exclusive with all other wrapper modes. `notices.py --features feishin-native-spike/gst-probe OUTPUT` collects the selected optional dependency notices. See [commands and measured limits](../../docs/app/p01/gstreamer-candidate.md).

`--prepared-handoff` runs the ignored real mpv local preparation probe; `--gstreamer-handoff` runs the optional cross-engine preparation probe. Both require external mpv and null output; the latter also needs GStreamer prerequisites. They preserve failures/annotations without changing existing mode scopes. See the [prepared source guide](../../docs/app/p01/prepared-handoff.md).

`native_notices.py` inventories staged AppDir ELF identities, original/bundled hashes and exact installed package/copyright provenance. Development AppImage packaging now requires Ubuntu 24.04, dpkg-query, ldconfig and readelf (binutils), and verifies the inventory after its output pass. [Native artifact provenance](../../docs/app/licenses/native-artifacts.md) records actual extraction/runtime checks and the remaining licensing/source obligations. This does not change normal Rust build support or certify a release.

`p03-playlist-reference.cjs` verifies pinned source/compiler hashes before replaying only selected original playlist/song expressions and controllers against an in-memory API double. It requires explicitly supplied optional source/compiler paths; default builds and CI use tracked Rust fixtures and integrity checks. See [playlist verification](../../docs/app/p03/playlist-reads-verification.md).

`p03-collection-reference.cjs` additionally binds original API route method/path declarations while replaying selected folder/role/random/similar-song controller expressions. Explicit optional source/compiler inputs never become build/CI dependencies. See [collection checks](../../docs/app/p03/collection-reads.md).

`p03-list-reference.cjs` replays selected original playlist/genre list/count/normalizer/slicing expressions with hash-bound Lodash and in-memory endpoint doubles. Explicit source/compiler/Lodash inputs remain optional local evidence. See [local list checks](../../docs/app/p03/local-lists.md).

`p03-enrichment-reference.cjs` replays selected original artist-info and album/artist radio expressions with in-memory HTTP-status/body fixtures. Explicit optional source/compiler inputs never become build dependencies. See [enrichment checks](../../docs/app/p03/enrichment-reads.md).

`p03-artist-reference.cjs` replays selected original artist/album-artist list/count, normalizer and shared-sort expressions against in-memory indexes with pinned Lodash. Source enum aliases are read as literals from hash-verified input. See [artist checks](../../docs/app/p03/artist-lists.md).

`p03-album-count-reference.cjs` replays only the original album-count controller and first-folder helper with finite in-memory response/header fixtures and explicit calendar samples. Source constants/routes are extracted from hash-verified inputs. See [count checks](../../docs/app/p03/album-counts.md).

`p03-album-list-reference.cjs` replays selected original album-list/normalizer/sort expressions with captured calendar years and pinned Lodash RNG contexts. It reads source mappings/enum aliases as literals, temporarily scopes its process timezone and records a second-zone probe. Source/TypeScript/Lodash inputs remain optional. See [album-list checks](../../docs/app/p03/album-lists.md).

`p03-song-count-reference.cjs` replays the original song-count controller and first-folder helper using finite synthetic responses. Hash-verified source/compiler inputs remain optional; constants and five API routes are bound to the fixture. See [song-count checks](../../docs/app/p03/song-counts.md).

`p03-search-reference.cjs` replays selected original combined-search/normalizer expressions and binds search3 routing. Explicit hash-verified source/compiler inputs remain optional. See [search checks](../../docs/app/p03/search.md).

`p03-song-list-reference.cjs` replays the original song-list/normalizer/sort expressions against finite endpoint doubles with pinned Lodash RNG contexts and a restored process timezone. Explicit external inputs remain optional. See [song-list checks](../../docs/app/p03/song-lists.md).

`p03-catalog-reference.cjs` replays original favorite/top composition plus radio/scan controllers and feature membership using hash-bound source/TypeScript/Lodash. Default CI requires only tracked fixtures. See [catalog checks](../../docs/app/p03/catalog-reads.md).
