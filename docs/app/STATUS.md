# Project scratchpad

- Stage: P01.01 complete; P01.02-.08 have prototypes/evidence but open exit gates. No P02+ implementation.
- Native UI: pinned Rust 1.92 / egui 0.33.3; Glow/wgpu candidates, separate web/remote WASM roots; keyboard Tab/arrows/Home/End and named controls.
- Table: 100k virtual rows, bounded worker/artwork; multi-track/album-block moves with cancellation and stale-view rejection; 6000 CPU-only concurrent scrolling/sort/filter/group/playback/monitor samples, not presented frames.
- Media: Unix mpv plus cross-checked Windows named-pipe candidate (runtime pending); explicit ffplay/SDL and offline FFmpeg filter/waveform candidates, not a selected production fallback.
- Capture: one-slot off-UI supervisor with generation isolation, suspend/re-enable/retry and real owned-monitor checks; no default host capture.
- Browser: real disposable Navidrome FLAC/MP3 on three lab engines; six session callbacks, cancellable/deadlined play, lifecycle/reload checks; Firefox/WebKit seeks remain open.
- Packaging/accessibility: rebuilt Ubuntu x86_64 AppImage runs without checkout/Rust; Linux AT-SPI names audited after fixing unnamed filter; no screen-reader/device certification.
- Hosted a15c9d7: native Linux/Windows/macOS, portable/WASM and aarch64 AppImage passed; x86_64 package provisioning still running; not all green.
- Latest checks: 32 default Rust, 49 Python with reference; five external native probes via wrapper passed on Ubuntu x86_64; format/default-WASM-wgpu Clippy and release web builds passed; older runtime manifests stay historical.
- Records: [continuation](p01/continuation.md), [matrix](p01/README.md), [migration map](migration/p01-map.csv), [license gaps](licenses/README.md), [Windows IPC candidate](p01/windows-ipc.md).
- Active: diagnostics pushed as a15c9d7; Windows IPC candidate and four Windows-only tests compile; Unix/default/target/notice checks passed; both reviews accepted the ownership fix.
- Open: calibrated physical budgets, renderer/input/IME/RTL/touch/screen-reader/OS matrix, Windows runtime/OS controls/capture, full native fallback and release licensing; no P01 exit.
- Next: commit/push Windows boundary and inspect hosted results; P02 only after verified P01 exit.
