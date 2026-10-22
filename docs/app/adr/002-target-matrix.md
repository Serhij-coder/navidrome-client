# ADR-002: target scope and evidence matrix

Status: P00 scope recorded; candidate OS floors and validation matrix proposed, no target certified. Final support claims depend on P01/P13/P14 evidence.

Keep desktop, full browser and remote controller deliverables. Desktop is prioritized; prioritization does not defer other targets. The upstream `electron-builder.yml` declares Windows x64/arm64, macOS x64/arm64 and Linux packaging, with separate Linux arm64 workflow. Use that as scope evidence, not proof all machines run the rewrite.

| Deliverable | CPU / candidate validation environment | Required evidence |
| --- | --- | --- |
| Linux desktop | x86_64 and aarch64; Ubuntu 24.04 LTS clean images, X11 and Wayland; current Nobara development host supplementary | AppImage for each supported CPU, no checkout/toolchain, assets/playback, mpv/FFmpeg dependency statement, tray/MPRIS, screen reader and capture |
| Windows desktop | x86_64 and arm64; Windows 11 | Native playback/process pipes, media keys, DPI/IME/accessibility, installer and teardown |
| macOS desktop | x86_64 and arm64; macOS 14 or newer candidate floor | Intel/Apple GPU, permissions/capture, media controls, accessibility, installation/signing policy |
| Full web | wasm32; latest and previous stable Chromium/Firefox and Safari on supported macOS; versions recorded at test time | Actual authenticated streams, CORS/autoplay/codecs, persistence, subpath/bootstrap/PWA and media session |
| Remote controller | wasm32; above desktop browsers plus Android Chromium and iOS Safari current/previous stable | Five surfaces, touch/input, auth/reconnect, per-client replies, applied-operation acknowledgments |

No hardware/CI was available for Windows, macOS, aarch64 or mobile in this task. The browser automation inventory was empty. No operating-system floor or browser version is advertised as supported today. P01 must pin the actual toolchain, renderer and test versions after P00 calibration; revise this proposal if framework requirements conflict, preserving gaps rather than silently dropping targets.

Alternatives: Linux x86_64 only would reduce test cost but omit upstream CPU/platform scope; it is not adopted. One universal browser/desktop engine would simplify composition but cannot provide native OS behavior. WASM core must avoid native dependencies and universal `Send` future requirements. Remote controls desktop playback; it does not acquire local queue ownership.

P01 update (2026-10-22): the maintainer explicitly authorized workspace/feasibility work ahead of P00 exit. The [task matrix](../p01/README.md) and [verification record](../p01/verification.md) contain actual builds/probes and open acceptance cases. P00 calibration and the platform/accessibility/native-fallback gates are not waived. Glow remains a provisional development default; software-rendered launches do not select a production renderer or certify a platform. Unix mpv, owned Linux monitor capture and browser fixtures establish bounded feasibility only; Windows named-pipe/OS media controls and native fallback remain unresolved. [ADR-008](008-visualization-feasibility.md) records the analysis/preset boundary.
