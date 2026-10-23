# P01 workspace and feasibility prototypes

P01 started at the maintainer's explicit request on 2026-10-22 while [P00 gates](../baseline/README.md) remain open. This is a runnable feasibility workspace, not a music client or a P01 exit. No P02 or later implementation started. The [verification record](verification.md) separates executed checks from required missing evidence. No toolkit/platform/audio/accessibility parity is inferred from a build.

The [continuation record](continuation.md) adds keyboard/accessibility, browser session/lifecycle, native candidates and concurrent measurements, plus actual hosted CI outcomes. Earlier evidence hashes remain historical snapshots.

## Implemented slice and open gates

| Task | Implemented now | Remaining acceptance evidence |
| --- | --- | --- |
| P01.01 | Cargo workspace/lockfile; Rust/MSRV 1.92.0, edition 2024; formatter, Clippy, GPL package metadata; explicitly wired tests | Complete for this initial workspace; dependency changes must keep the pinned compiler passing |
| P01.02 | eframe/egui 0.33.3 desktop; Glow default candidate and optional wgpu 27.0.1; separate full-web/remote WASM composition roots | Physical GPU/driver comparison, native platform launches and full browser/touch matrix; no measured production renderer decision |
| P01.03 | 100k native synthetic records; virtual rows, range/toggle selection, Tab/arrows/Home/End, column width controls/header drag, album groups, multi-record drag and whole-album block drag with same-album track moves and session reset on query changes; latest-only native metadata sort/filter/group/reorder worker with cancellation; owned PNG decode batches <=32 and a 64-texture cache; no periodic quiescent table repaint | Real-library sorting/network artwork churn; CPU-only concurrent playback/artwork/worker samples do not certify input-to-present timing, calibrated frame/RSS/CPU/GPU/startup budgets, retention, unfocused/minimized/tray-only measurements; browser fixture capped at 1k, remote at 100 |
| P01.04 | Browser media element and WebAudio; explicit Play gesture, actual disposable Navidrome FLAC/MP3; bounded cancellable play and six Media Session callback actions, CORS/codec/error handling; no browser imports in native UI | Real music-server authenticated codecs/transcodes, cookie/credential CORS policy, blocked-autoplay scenarios and device output; OS media-key interaction, [Firefox/WebKit boundary and looping limits](browser-seeking.md), next/real metadata, Safari/current/previous/mobile matrix |
| P01.05 | Unix mpv process/JSON IPC and cross-checked Windows named-pipe candidate; local current/next, pause/seek, lowpass, request IDs/deadlines, malformed/oversized/stale reply rejection, generation check, process death, bounded shutdown/socket cleanup; explicit ffplay/SDL and offline FFmpeg filter/waveform candidates | Real Windows mpv/device validation, OS media control routing, native fallback, gapless/crossfade timing, full DSP/device/waveform requirements, authenticated streams; no fallback engine is selected |
| P01.06 | Named filter/CSS/scale controls, keyboard-only table, isolated AT-SPI tree and [Orca selection announcements](screen-reader.md); editable text/RTL layout probe, row context menu/clipboard and inert CSS report; initial executable x86_64 AppImage built on Ubuntu 24.04 and launched in a separate runtime container with no Rust/check-out | Complete screen-reader/keyboard/focus journeys across targets, RTL shaping/fonts, IME, mixed DPI, clipboard, touch/text input and physical target matrix; current aarch64 artifact/full runtime certification, physical audio/device evidence and complete distribution notices |
| P01.07 | Tracked-input CI definitions: portable/tooling/native/WASM, both Linux AppImage architectures; structured allowlisted JSON diagnostics; web/AppImage builders and dependency/font notice inventory | P01.07 complete for tracked-input CI establishment: [58814c1 passed all seven jobs](evidence/hosted-windows-ipc.json), including native Linux/Windows/macOS, portable/WASM and both AppImage architectures; full target/device certification remains separate; external toolchain/packaging prerequisites remain necessary |
| P01.08 | Original bounded spectrum/waveform preset; 256 finite/clamped samples, 16 frequency bins; one-slot analysis; explicit Linux monitor capture; owned mpv-loopback and unavailable-source/disposal tests; bounded off-UI capture supervisor with retry/re-enable; browser analyser lifecycle experiment | Native/browser preset rendering/resource measurements, actual OS denial prompts and capture APIs on each OS, complete engine-switch lifecycle, preset compatibility/bridge decision; no Butterchurn compatibility claim |

The prototypes deliberately do not implement authentication adapters, a production queue/state owner, durable settings/imports, server browsing, remote protocol/service or deployment. Those remain their existing phases. Dragging selected records or album headers changes session view order. Successive drops compose; sort/filter/group changes or Reset order restore metadata order. Selection is keyed by synthetic record identity. Grouped track moves remain within one album, and album headers move entire blocks. Stale/pending views and cross-album track moves are rejected; no library metadata or queue is changed. Metadata sorting is real worker work (covered with non-monotonic titles and non-contiguous album inputs). The default fixture uses monotonic synthetic titles, so its timings still do not represent a realistic library sort. The artwork workload decodes only an original embedded 32x32 PNG, with latest-only batches and a 64-entry LRU texture cache; remote fetch and arbitrary image bounds remain later adapter/UI work.

## Setup and build

Rust is pinned in [rust-toolchain.toml](../../../rust-toolchain.toml); the workspace MSRV is the same 1.92 compiler. Install rustup using the official installer, then from the repository root:

```sh
rustup toolchain install 1.92.0 --profile minimal --component rustfmt,clippy --target wasm32-unknown-unknown
cargo build --workspace --locked
cargo run -p feishin-desktop --locked
cargo run -p feishin-desktop --release --no-default-features --features wgpu --locked -- --wgpu
```

Nobara 44/Fedora development prerequisites (the project does not execute sudo):

```sh
sudo dnf install gcc gcc-c++ pkgconf-pkg-config libX11-devel libXcursor-devel libXi-devel libXrandr-devel libxkbcommon-devel libxkbcommon-x11 wayland-devel mesa-libEGL-devel mesa-libGL-devel alsa-lib-devel openssl-devel dbus-devel mpv ffmpeg file xorg-x11-server-Xvfb fuse fuse-libs
```

Ubuntu 24.04 CI uses `libx11-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libwayland-egl1 libxcursor1 libxi6 libxrandr2 libxfixes3 libxrender1 libegl1-mesa-dev libgl1-mesa-dev libdbus-1-dev xvfb xauth mpv ffmpeg pulseaudio file libfuse2t64`. A working display/GPU driver is a runtime prerequisite. Xvfb/software Vulkan is a diagnostic environment, not the physical performance reference. Keep development data isolated: the current shell does not load/save profiles, credentials or settings. Never give it the original Feishin profile.

Native and WASM dependencies are separated at target/feature boundaries. `feishin-spike-model` contains only portable synthetic view/selection/analysis code; it is not the P02 domain. `feishin-spike-ui` owns views; `feishin-native-spike` owns blocking external processes; `feishin-browser-spike` owns WASM-only browser bindings. Composition roots are `apps/desktop`, `apps/web`, `apps/remote`. Desktop `glow`/`wgpu` features may be compared together, but the explicit commands above exercise each separately. Do not use an unqualified all-features target matrix as evidence.

## Checks and experiments

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo clippy -p feishin-web -p feishin-remote --target wasm32-unknown-unknown --locked -- -D warnings
cargo check -p feishin-spike-model --target wasm32-unknown-unknown --locked
python3 tools/app/check-p00.py
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Tests live under top-level `tests/`, wired by Cargo test entries or private `cfg(test)` includes. Default Cargo tests run no audio capture and require Python 3 on Unix for the fake IPC peer. Native real-engine tests and the release benchmark are explicitly ignored by default. Optional P00 reference checks stay separate and never become application build/CI inputs.

```sh
cargo test -p feishin-native-spike --test native real_mpv_current_next_pause_seek_filter_and_process_death -- --ignored
cargo test -p feishin-native-spike --test native real_ffmpeg_analysis_and_disposal -- --ignored
cargo test -p feishin-native-spike --test native unavailable_monitor_exits_without_a_frame -- --ignored
P01_BENCH_OUTPUT=/tmp/p01-new-samples.json cargo test -p feishin-spike-ui --release --test headless_performance -- --ignored
```

The benchmark refuses to overwrite its output. It saves raw 30-sample synthetic view operations and 6000 CPU-only stationary layout samples. It measures neither actual scrolling nor presented frames, audio, artwork, hardware input, startup, RSS or GPU cost. Do not compare it to the P00 acceptance frame budget.

For explicit local owned-tone playback:

```sh
cargo run -p feishin-desktop -- --tone /tmp/p01-new-owned.wav
cargo run -p feishin-desktop -- --mpv-probe mpv /tmp/p01-new-owned.wav
cargo run -p feishin-desktop -- --mpv-probe-null mpv /tmp/p01-new-owned.wav
```

`--mpv-probe-null` discards audio and validates IPC only. The probe supports Unix sockets and a [Windows named-pipe candidate](windows-ipc.md); Windows hosted fixture tests passed for `58814c1`; real mpv/device verification remains open. All calls remain blocking worker/CLI operations. `--analysis-test-tone` visualizes an FFmpeg-generated tone. `--capture-monitor SOURCE` explicitly opts into that Linux PulseAudio/PipeWire source; it never auto-selects a microphone or monitor. Closing/minimizing the window or disabling visualization disposes capture on a bounded worker. Re-enable reuses the explicitly authorized session source; failure requires off/on retry. Final shutdown joins owned work. Playback experiments have their own process lifetime and do not run in paint callbacks. OS media controls are not integrated with an application owner yet; mpv's native media-key setting is only a candidate probe.

Only use the owned-monitor test with a separately provisioned, disposable `p01_owned` null sink and `PULSE_SERVER` pointing at that isolated server:

```sh
P01_CAPTURE_MONITOR=p01_owned.monitor cargo test -p feishin-native-spike --test native owned_mpv_monitor_capture_and_teardown -- --ignored
```

Do not run `-- --ignored` indiscriminately: it also includes this explicitly authorized capture test. `P01_MPV` can select a trusted mpv executable. Tools launch executables directly without a shell; never feed an untrusted executable path. Only local files are supported by this native spike. Child stderr/protocol payloads and error paths/URLs are excluded from app diagnostics. The logger emits only component/event/generation; test fixtures check redaction. Engine generations here prove lifecycle filtering helpers, not a production engine event bus.

## Browser and remote bundles

```sh
cargo install wasm-bindgen-cli --version 0.2.106 --locked
python3 tools/app/build-web.py
python3 -m http.server 8091 --bind 127.0.0.1 --directory target/p01-web
```

Open `http://127.0.0.1:8091/web/` and `/remote/`. The remote owns no local media or desktop queue and is a presentation experiment only. Both roots reuse egui; full native 100k background processing is not asserted for the bounded browser fixture. The full-web page accepts absolute HTTP(S) token URLs with no embedded username/password, held in tab memory only. Anonymous CORS excludes cross-origin cookies; token query URLs need appropriate server CORS. Do not capture real credential URLs in logs/screenshots. The input is masked; browser network tools can still see the requested URL.

Play resumes WebAudio within the user gesture. Native audio-element controls provide basic pause/seek. Browser media errors are generic so URLs/tokens do not enter app output. Optional Media Session APIs are caught; unsupported browsers retain the visible controls. Analysis runs only for a visible, playing consumer and stops its interval when disabled/paused/hidden. The representative preset uses PCM analysis rather than copied upstream JS. Diagnostic WASM exports expose only visualization visibility and a numeric analysis peak.

The optional [browser scenario](../../../tests/browser/p01-browser.cjs) needs externally installed Playwright 1.56.1 and matching Chromium/Firefox/WebKit libraries. It serves only generated bundles and its own token-gated WAV, including wrong token, CORS denial and malformed codec. Run in isolated infrastructure, with headed Firefox under Xvfb for software WebGL and a functioning owned audio service. The [seek comparison](browser-seeking.md) documents the Linux null-sink setup and separate open seek gates:

```sh
mkdir -p /tmp/p01-browser-results
PULSE_SERVER=unix:/absolute/path/to/owned/pulse.sock P01_PLAYWRIGHT=/absolute/path/to/playwright xvfb-run -a node tests/browser/p01-browser.cjs target/p01-web /tmp/p01-browser-results
```

## Development AppImage and notices

Build on the oldest intended distribution, not on a newer host and assume backward glibc compatibility. P01 generated a host-built smoke artifact, then built x86_64 on Ubuntu 24.04 from a clean source copy and tested it in a separate Ubuntu runtime without a checkout or Rust. Shell/font startup, extracted resources/notices and an external null-output mpv probe passed after explicitly bundling runtime-loaded libraries. This is not physical audio output or complete Linux packaging. The tracked CI defines Ubuntu 24.04 x86_64/aarch64 builds and off-checkout smoke runs; the initial jobs passed, while current revision and full platform certification remain separate. It does not upload artifacts or publish a release.

```sh
cargo build -p feishin-desktop --release --locked
curl --fail --location https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage --output /tmp/linuxdeploy.AppImage
chmod +x /tmp/linuxdeploy.AppImage
python3 tools/app/package-appimage.py --linuxdeploy /tmp/linuxdeploy.AppImage
APPIMAGE_EXTRACT_AND_RUN=1 target/appimage/Feishin-Rust-P01-x86_64.AppImage --smoke
```

The packaging script verifies the exact linuxdeploy digest before execution and refuses artifact overwrite. Use the matching aarch64 URL on that architecture. A changed continuous download fails closed until its new bytes/version are reviewed. `APPIMAGE_EXTRACT_AND_RUN=1` avoids requiring a mounted FUSE filesystem for smoke tests. mpv/FFmpeg remain external optional probe prerequisites. No native binary is silently bundled. The builder explicitly includes the winit/clipboard X11/Wayland/xkbcommon dlopen libraries, rather than relying only on ELF scanning. External engine behavior, broader dlopen coverage, distribution floors and runtime/library notices must be audited before releases. [Notice provenance](../licenses/README.md) describes exact retrieved texts and remaining gaps. The generated icon is original development artwork, not Feishin release branding.

## Troubleshooting and next work

Use the default Glow build first. The Xvfb host needed `WGPU_BACKEND=vulkan` and the correct Mesa software ICD for wgpu; its default driver path failed. This is recorded evidence, not permission for a silent renderer switch. On a real display compare both backends and save GPU/driver/display/power details before selecting the production renderer. A missing device/screen reader/target result stays open.

Continue only P01: complete the open gates above and [ADRs 001/003/008](../adr/001-egui-desktop.md). Calibrate P00 budgets, run physical accessibility/input and target matrices, evaluate the native replacement/fallback strategy and realistic concurrent workloads. P01 exit and later bulk screens remain blocked by those missing results. [Source migration mapping](../migration/p01-map.csv) records prototypes as deferred production work, not ports.
