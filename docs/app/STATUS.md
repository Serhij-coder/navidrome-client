# Project scratchpad

- Stage: P01.01/P01.07 complete; P01.02-.06 and P01.08 have prototypes/evidence but open exit gates. No P02+ implementation.
- Native UI: pinned Rust 1.92 / egui 0.33.3; Glow/wgpu candidates, separate web/remote WASM roots; keyboard Tab/arrows/Home/End and named controls.
- Table: 100k virtual rows, bounded worker/artwork; multi-track/album-block moves with cancellation and stale-view rejection; 6000 CPU-only concurrent scrolling/sort/filter/group/playback/monitor samples, not presented frames.
- Media: Unix mpv plus cross-checked Windows named-pipe candidate (hosted fixture tests passed; real mpv pending); explicit ffplay/SDL and offline FFmpeg filter/waveform candidates, not a selected production fallback.
- Capture: one-slot off-UI supervisor with generation isolation, suspend/re-enable/retry and real owned-monitor checks; no default host capture.
- Browser: real disposable Navidrome FLAC/MP3 on three lab engines; six session callbacks, cancellable/deadlined play, lifecycle/reload checks; default interior seeks pass in the lab; Firefox/WebKit boundary/loop limits remain open.
- Packaging/accessibility: rebuilt Ubuntu x86_64 AppImage runs without checkout/Rust; nine-step Orca/eSpeak NG selection journey passed; no physical/device certification.
- Hosted daca831: all seven jobs passed, including Windows position assertions and both AppImage CPUs; earlier 55651db native failure remains unexplained, not a current CI failure. No P01 exit implied.
- Latest checks: 54 default Rust; 68 Python with reference; five external native probes via wrapper passed on Ubuntu x86_64; format/default-WASM-wgpu Clippy and release web builds passed; older runtime manifests stay historical.
- Records: [matrix](p01/README.md), [Orca/CI](p01/screen-reader.md), [Windows IPC](p01/windows-ipc.md), [native transition](p01/native-transitions.md), [migration](migration/p01-map.csv), [licenses](licenses/README.md).
- Delivered P01.06 slice: Shift+F10 copy/focus and fresh menu-item IDs; local-display/session guard fixed; 15 clipboard/19 Orca checks rerun; 54 Rust/68 tooling and target Clippy/reference/link checks passed; both reviews satisfied. See [record](p01/track-actions.md).
- Open: calibrated physical budgets, renderer/input/IME/RTL/touch/screen-reader/OS matrix, Windows runtime/OS controls/capture, full native fallback and release licensing; no P01 exit.
- P01.04: shared element/callback seek deadlines and fresh play status; all 54 lifecycle checks passed after review assertion fix; both reviews satisfied; native hosted failure stays open.
- Delivered P01.05: bounded optional position query and paused readiness/seek probes implemented; 49 Rust/64 tooling tests, five real native probes and calibration/3-curve/19-transition capture regressions passed; both reviews satisfied; see [record](p01/native-position.md).
- Next: native control routing/fallback and remaining performance/input/capture/platform P01 gates; P02 only after verified P01 exit.
- Workflow: project-local Sol high orchestration, low/medium implementation, Luna xhigh research and two Sol medium reviews configured; see [setup and validation](development-workflow.md). No app phase gate changed.
