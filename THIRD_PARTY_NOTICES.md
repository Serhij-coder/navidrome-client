# Third-party notices and release obligations

This project is an independent Rust rewrite inspired by and based on [Feishin](https://github.com/jeffvli/feishin), originally developed by jeffvli and contributors. The source baseline is commit `7d6ad318c79802e226996cfc39acba6b05bb22e8` (development, package 1.17.0). Preserve their attribution and the GNU General Public License version 3 in [LICENSE](LICENSE). The root license is byte-identical to the archived upstream license (SHA-256 `8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903`).

P00 contains analysis, synthetic fixtures and tooling, with source-derived behavior records attributed above. No upstream fonts, icons, media, native binaries or application runtime libraries have been copied into the tracked Rust application. The ignored source archive and the temporary reference dependency installation are development evidence, not release artifacts. There is no Rust dependency lockfile or distributed application yet.

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

Upstream runtime dependencies include TagLib WASM, Butterchurn/presets, audioMotion, Japanese conversion/dictionaries, React/Mantine and other packages in its frozen pnpm lock. They have not been selected as Rust dependencies or authorized as hidden native bridges. Any future retained component needs exact version, license, notices, source obligations and a measured boundary. No native `.so`, `.dll`, `.dylib`, `.exe`, `.node` or `.wasm` files were found in the archived tracked source; downloaded dependencies/build output are a separate scope.

mpv is external in the reference; FFmpeg may be needed for casting/transcoding. Before packaging either executable/library, inventory the actual build configuration/codecs and licensing, provide required notices/source obligations, and document whether the binary is bundled or an external prerequisite. Do not infer a single license from a project name. Every release must include this project's LICENSE and complete asset/dependency/native-binary notices; Linux additionally requires executable AppImages and clean-environment startup/assets/playback checks per supported CPU.
