# Third-party notices and release obligations

This project is an independent Rust rewrite inspired by and based on [Feishin](https://github.com/jeffvli/feishin), originally developed by jeffvli and contributors. The source baseline is commit `7d6ad318c79802e226996cfc39acba6b05bb22e8` (development, package 1.17.0). Preserve their attribution and the GNU General Public License version 3 in [LICENSE](LICENSE). The root license is byte-identical to the archived upstream license (SHA-256 `8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903`).

P00 contains analysis, synthetic fixtures, original-UI screenshot/DOM evidence and tooling, with source-derived behavior records attributed above. No upstream fonts, icons, media, native binaries or application runtime libraries have been copied into the tracked Rust application. The ignored source archive and the temporary reference dependency installation are development evidence, not release artifacts. P01 subsequently adds a pinned Rust dependency lockfile and local development smoke bundles; no release is published. See the [P01 dependency/font notice provenance and remaining release gates](docs/app/licenses/README.md).

The [asset/configuration ledger](docs/app/baseline/assets.csv) records 443 source paths with hashes and explicit deferral boundaries. Deferred does not mean unused or parity complete. Source files outside that ledger remain in the immutable inventory and require eventual F52 dispositions. Before reusing/distributing an asset, obtain its actual license text, copyright/reserved-name obligations and provenance; package the required notices. Do not treat the repository GPL as proof of every embedded asset's independent license.

Font metadata inspected with fontTools 4.62.1 on the archived WOFF2 files:

| Asset family | Embedded copyright / license indication | Distribution gate |
| --- | --- | --- |
| Inter | Inter Project Authors, 2016; SIL Open Font License 1.1, openfontlicense.org | Preserve copyright, full OFL text and any reserved names |
| Poppins (8 weights/styles) | Poppins Project Authors, 2020; SIL Open Font License 1.1, scripts.sil.org/OFL | Same; verify each exact asset before copying |
| Noto Sans Hebrew | Noto Project Authors, 2024; SIL Open Font License 1.1 | Same |
| Noto Sans JP | Adobe, 2014-2021; SIL Open Font License 1.1; reserved name Source | Verify exact provenance and reserved-name terms |
| Symbola | George Douros, 2016; embedded statement "Fonts in this site are free for any use." and historical author URL | Verify authoritative redistribution terms or replace; this statement alone is not a complete reviewed license |

These are embedded metadata observations, not a completed legal/distribution review. No separate font license files were found under upstream `assets/`. Feishin icons/branding and preview media also need reuse/replacement decisions; no new release identity is adopted here.

P00.08 now retains [per-file font hashes/version/copyright/license metadata and publisher license snapshots](docs/app/baseline/font-license-evidence.json), including full original OFL text in JSON evidence containers under `docs/app/baseline/licenses/`. These are notice evidence only; no font binary was added or selected. Publisher snapshots are from Google Fonts on 2026-10-22, not pinned copies of the exact archived binaries' distribution packages. Inter and Hebrew publisher copyright years differ from their embedded metadata; preserve that distinction and reconcile provenance before reuse. Poppins and Noto Sans JP text/metadata indications agree in the inspected copyright notices. Symbola remains unselected pending authoritative terms or a replacement with verified character coverage. Do not turn an unavailable font license into a loss of supported text rendering.

Upstream runtime dependencies include TagLib WASM, Butterchurn/presets, audioMotion, Japanese conversion/dictionaries, React/Mantine and other packages in its frozen pnpm lock. They have not been selected as Rust dependencies or authorized as hidden native bridges. Any future retained component needs exact version, license, notices, source obligations and a measured boundary. No native `.so`, `.dll`, `.dylib`, `.exe`, `.node` or `.wasm` files were found in the archived tracked source; downloaded dependencies/build output are a separate scope.

The disposable Jellyfin/Navidrome/Gonic container images used for P00 protocol captures are local test infrastructure, not application dependencies or release assets. Image identities and boundaries are recorded in the [fixture runbook](docs/app/baseline/live-servers.md). They must not be bundled into a release by implication.

mpv is external in the reference; FFmpeg may be needed for casting/transcoding. Before packaging either executable/library, inventory the actual build configuration/codecs and licensing, provide required notices/source obligations, and document whether the binary is bundled or an external prerequisite. Do not infer a single license from a project name. Every release must include this project's LICENSE and complete asset/dependency/native-binary notices; Linux additionally requires executable AppImages and clean-environment startup/assets/playback checks per supported CPU.

The [P00 screenshot set](tests/fixtures/p00/browser/2026-10-22) captures the original Feishin UI, icons/branding and rendered fonts for reference characterization. These captures are attributed to jeffvli/Feishin and contributors under the upstream license; original font notices and unresolved exact-asset provenance are recorded above. The captures do not select any original UI asset or font for the Rust application.

P01 uses egui/eframe 0.33.3, optional wgpu 27.0.1 and the exact Rust dependency versions in Cargo.lock. Generated bundles include project notices, dependency license/manifests/readmes, toolchain terms and nested default-font texts. Exact upstream-revision overrides and clearly labeled SPDX-template gaps are recorded in `docs/app/licenses/registry-overrides.json`. Native AppImage runtime/system-library obligations and missing exact package notices remain release gates; this inventory does not certify release readiness. The P01 development icon is original GPL-3.0-only artwork. No Feishin font/icon/media binary was selected for these shells.

P01 gain-envelope formulas in `crates/spike-model/src/fade.rs` are adapted from the pinned Feishin active web player and volume utility, attributed to jeffvli and contributors under the project GPL-3.0 license. The tracked source-derived numeric fixture records exact function/file hashes and the observed exponential quirk. It contains no upstream font, media or native-binary asset.

The opt-in Linux GStreamer feasibility helper uses gstreamer-rs 0.24.4 (MIT OR Apache-2.0) and exact transitive versions in Cargo.lock. Feature-scoped registry notices and external native plugin/package evidence are recorded in the [candidate guide](docs/app/p01/gstreamer-candidate.md). GStreamer core/plugin/codec licenses and source obligations depend on the actual native build. Neither native GStreamer nor the helper is bundled in the default development AppImage; production selection and distribution auditing remain open.

P02 credential scoping uses sha2 0.10.9; optional `feishin-storage/linux-secret-service` uses secret-service 5.2.0 with Rust crypto and zbus 5.19.0 (MIT OR Apache-2.0), plus exact Cargo.lock dependencies. Generate notices with that same explicit feature selection when distributing it. The owned GNOME Keyring/D-Bus test environment is external test infrastructure, not a bundled application service. See [credential checks and limits](docs/app/p02/credential-verification.md).

P03.01 adapts Subsonic URL/authentication rules from the pinned Feishin controller/client and URL/random-string helpers, attributed to jeffvli and contributors under GPL-3.0-only. The portable request builder uses RustCrypto md-5 0.10.6 (MIT OR Apache-2.0) for the protocol-mandated salted token, with exact dependencies in Cargo.lock; MD5 is not selected for password storage or a new cryptographic protocol. The optional source replay uses frozen upstream qs 6.15.2, md5 2.3.0, charenc/crypt 0.0.2 and is-buffer 1.1.6 in a controlled external reference environment. Those JavaScript packages are test infrastructure and are not bundled. See [P03 scope and provenance](docs/app/p03/README.md).

P03.02 also adapts Subsonic envelope validation, protocol error categories, conditional extension discovery and form-request rules from the same pinned Feishin client/controller and shared wire schemas under GPL-3.0-only. Its synthetic tests and source anchors are documented in [the protocol record](docs/app/p03/protocol-verification.md); no additional registry package or external runtime is introduced.

P03.03 adapts song normalization and partial-date helper rules from the same pinned Feishin shared Subsonic normalizer/controller under GPL-3.0-only. The controlled source replay and synthetic expected outputs retain source/expression hashes in [song-read verification](docs/app/p03/song-reads-verification.md); no new distributed dependency is added.

The P03.03 album/artist detail continuation also adapts the pinned Feishin album, artist, release-type and nested-disc normalization rules under GPL-3.0-only. Original-controller replay and synthetic output provenance are recorded in [album verification](docs/app/p03/album-reads-verification.md). No new distributed dependency is added.

The P03.03 playlist read continuation adapts pinned Feishin playlist metadata and ordered-entry controller/normalizer rules under GPL-3.0-only. Synthetic output provenance and original-expression replay are recorded in [playlist verification](docs/app/p03/playlist-reads-verification.md). No new distributed dependency is added.

The P03.03 collection continuation adapts pinned Feishin folder/role/random/similar-song controllers and shared song rules under GPL-3.0-only. [Collection verification](docs/app/p03/collection-reads.md) binds original expressions and route declarations; no new distributed dependency is added.

The local P03 playlist/genre list continuation adapts pinned Feishin list/count/normalization/slicing rules under GPL-3.0-only. Optional source replay uses hash-bound Lodash4.17.21 (MIT), already reference infrastructure rather than a distributed dependency. See [list verification](docs/app/p03/local-lists.md).

The P03 enrichment continuation adapts pinned Feishin artist-info and album/artist-radio controller rules and shared song normalization under GPL-3.0-only. [Enrichment verification](docs/app/p03/enrichment-reads.md) records original-expression provenance; no new distributed dependency is added.

The artist-list continuation adapts pinned Feishin artist/album-artist list/count and shared sorting/normalization rules under GPL-3.0-only. [Artist verification](docs/app/p03/artist-lists.md) records original-expression provenance; no distributed dependency is added.

The album-count continuation adapts pinned Feishin count branching, pagination, header and first-folder rules under GPL-3.0-only. [Count verification](docs/app/p03/album-counts.md) binds original expressions, constants and routes; no distributed dependency is added.
