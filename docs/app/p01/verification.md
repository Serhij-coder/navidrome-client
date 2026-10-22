# P01 initial workspace verification - 2026-10-22

This record covers a coherent initial workspace/feasibility increment, not P01 completion. The maintainer requested starting P01 while P00 remained open. [The task matrix](README.md) retains every subphase and its unverified acceptance cases. No production domain/persistence/backend/queue/remote implementation or later phase is claimed.

## Environment and evidence

Rust 1.92.0 (`ded5c06cf`, 2025-12-08), egui/eframe 0.33.3, optional wgpu 27.0.1; complete dependencies are pinned by Cargo.lock. The development host is x86_64 Nobara 44, AMD Ryzen 7 7435HS, 16 GiB RAM. Toolchain/bootstrap dependencies were installed in isolated temporary task directories, not made hidden repository prerequisites. The documented rustup/system prerequisites reproduce setup from a checkout.

Native launches used a disposable Xvfb 1280x900x24 display and software rendering, not the physical display/GPU/audio device. Glow started through Mesa EGL. wgpu required explicit Vulkan and the correct software ICD (`WGPU_BACKEND=vulkan`, `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json` on this host). The default wgpu driver path failed; this remains driver-path evidence, not a production backend selection. Unfocused/minimized/tray CPU/GPU and real display latency are unmeasured. The `--hidden-smoke` test is a synthetic hidden-window/disposal check with a close timer, not a quiescent tray benchmark.

Native engine tests used mpv 0.37.0 / FFmpeg 6.1.1 libraries from Ubuntu 24.04 task infrastructure and the host FFmpeg tool, with isolated runtime library paths. Default real-mpv tests used null output and therefore prove IPC, not audible sound. The owned-monitor test used an explicitly selected PulseAudio null sink `p01_owned`, connected only through a task socket. Its generated 750 Hz tone reached analysis through mpv PulseAudio output and FFmpeg monitor capture. No host microphone, desktop monitor or live profile was opened. This is loopback signal evidence, not physical device/OS permission-prompt validation.

The [browser evidence](../../../tests/fixtures/p01/browser/2026-10-22/browser-results.json) uses Playwright 1.56.1, its Chromium 141.0.7390.37 / Firefox 142.0.1 / WebKit 26.0, software graphics and an isolated owned audio sink. These are pinned lab engines, not a latest/previous stable certification; WebKit on Linux is not macOS/iOS Safari. The optional scenario creates its own HTTP token-gated WAV and CORS/codec failures. All token values are public disposable fixture markers. No real music-server adapter, production credential, authenticated live stream URL or physical output is part of this evidence.

## Checks

Final implementation checks passed: 17 default Rust tests (four external native tests and one benchmark ignored), four explicitly invoked external native tests, one release benchmark, 45 Python tooling tests (38 passed/seven optional skips by default; all 45 passed with the frozen reference environment), and seven local reference-tool regressions. Formatting, both Clippy target graphs, renderer/target checks and the checks below passed. Hosted CI and physical target results remain unavailable. Reviews are recorded separately below.

The executed verification set is:

| Check | Scope |
| --- | --- |
| `cargo fmt --all -- --check` | All Rust source including wired top-level tests |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Native default feature graph |
| `cargo test --workspace --locked` | Portable selection/view/analysis, worker cancellation/shutdown, virtual table bounds/row stride/idle repaint and bounded artwork decode/cache, malformed/oversized/correlated IPC, cleanup, tone bounds/redaction; four external native tests and one benchmark explicitly ignored |
| WASM-target Clippy for `feishin-web`/`feishin-remote`, portable model check | Browser binding and portable dependency separation |
| wgpu-only Clippy/build check | Separate native renderer feature graph |
| Desktop cross-target checks | Windows x64 GNU/MSVC and arm64 MSVC, macOS x64/arm64 and Linux arm64 compile only; no link/launch/device result |
| `tools/app/build-web.py` | Both independently generated release WASM roots and notice inventories |
| Native release shell Glow/wgpu and hidden smoke | Process starts, builds rows/assets/fonts, closes; software renderer only |
| Four explicit external native tests | Real mpv current/next/pause/seek/lowpass/death; FFmpeg tone analysis; owned monitor capture/teardown; unavailable monitor exits without PCM |
| Browser scenario | Both roots, token rejection/accepted WAV progress, WebAudio nonzero analysis, CORS/codec errors, optional API setup; see exact saved per-browser outcomes |
| Ubuntu x86_64 AppImage build/separate runtime | Clean copied inputs on Ubuntu 24.04; separate Ubuntu runtime without checkout/Rust; shell/font startup, extracted GPL/font/std notices and bundled xkbcommon, owned tone generation and external null-output mpv probe; no hardware/arm64/release completion |
| Python tooling suite / evidence checker | Maintained artifact/link/source rules and packaging checksum/font/missing-license/path-boundary regressions |
| Clean-input copy build/checks | Tracked/intended application inputs only; no ignored archive/plan/inventory/tool dependency |
| Optional local reference checks | Inventory checker, seven analysis-tool regressions and inventory JavaScript syntax, separate from app CI |

[Raw CPU-only samples](evidence/headless.json) contain 30 samples each for 10k/100k reverse/filter/group and 6000 stationary layout calls after warmup. Row heights and viewport bounds are verified separately. These are not scrolling/presented frames or evidence that the P00 frame/memory/startup/input/audio budgets passed. The hardware/calibration/playback/artwork/retention gaps remain open. Do not turn percentile summaries of this synthetic test into a production performance claim.

## Corrections and review gate

Implementation checks exposed and fixed a web-sys Media Session binding mismatch, missing wgpu backend features, an analysis frequency-bin coverage error, virtual rows expanding vertically, and test-clock settling. Optional API registration now catches JavaScript exceptions, native renderer features enable their target backends, fixed row allocation is covered by a stride regression, and real FFmpeg plus model analysis tests cover the bin fix. A browser harness async polling error was corrected to poll a synchronous loaded-module predicate and assert a captured nonzero value; earlier apparent analysis passes were not retained as final evidence. Firefox requires a real isolated audio sink for its WebAudio context, rather than inferring analysis from its media clock. The app's capture path now explicitly distinguishes system/null/PulseAudio output in the engine probe.

A separate clean Ubuntu runtime first exposed a missing `libxkbcommon-x11` loaded via dlopen. The packager now resolves and explicitly bundles the native-architecture X11/Wayland/xkbcommon libraries, with a mixed-architecture/missing-library regression. The same clean runtime then passed shell startup, asset/notice extraction and the null-output mpv probe without a checkout or Rust. `file` was added as a packaging prerequisite after the clean builder exposed that missing tool.

The first local inventory run also scanned generated dependency Markdown under ignored `target/`. Its ignored local checker now excludes Cargo outputs, with a regression preserving checks of maintained app docs; those reference-tool changes stay local. Optional reproduction invocations initially used a missing esbuild path, then the wrong compiler version; they were rerun with the saved evidence's esbuild 0.25.12 and all 45 tests passed.

Exactly two independent reviewers inspected the implementation, test/CI/docs scope and final fixes. The correctness/architecture reviewer and the tests/edge-case/performance/operations reviewer both accepted the fixed code with no remaining actionable findings. Their initial findings were:

- P1: Focused table navigation contradicted egui's keyboard-input flag. Navigation now belongs to the focused table button, locks vertical arrows and cancels first-frame directional focus traversal. A real RawInput click/Down/Shift-Down regression also verifies that focusing the filter leaves selection alone.
- P1/P2: A partial-line peer could keep IPC alive beyond the deadline. Each incremental read and partial write now recomputes the remaining absolute command deadline; timeouts return `Deadline`. Slow startup and command peers, malformed/oversized frames, correlation, process death and real mpv tests pass.
- P2: Browser harness failures referenced a block-scoped variable and prevented evidence recording. The failure path now records each failed browser and continues; an explicitly fake launch-failure regression verifies all three records and excludes raw error payloads.
- P2: Non-Unicode local filenames and CLI arguments could panic. Checked path/argument conversion now returns controlled errors without echoing input. Both regressions pass.

Affected tests and complete native/WASM/wgpu Clippy, formatting, default workspace, Python/reference checks passed after the fixes. Both reviewers independently reran selected new regressions. The web roots were rebuilt and the three browser experiments rerun with nonzero WebAudio output, updating JSON and all six screenshots. Native Glow/software Vulkan/hidden smoke and all six cross-target compile checks passed again. The CPU-only benchmark was refreshed on the development host without calibration or controlled background load.

The fixed-source x86_64 AppImage was rebuilt from the clean copied inputs on Ubuntu 24.04. In the separate runtime with no checkout/Rust, shell startup, owned tone generation, external null-output mpv, bundled xkbcommon and extracted GPL/font/toolchain notices passed again. [The evidence manifest](evidence/checks.json) identifies the exact fixed sources, raw evidence and new artifact digest. Clean-copy offline workspace tests also passed. These reruns replace pre-review artifact evidence. No release, artifact upload or deployment is authorized by the Git commit/push workflow.
