# Project scratchpad

- Stage: P01.01/P01.07 complete; P01.02-.06 and P01.08 have prototypes/evidence but open exit gates. No P02+ implementation.
- Native UI: pinned Rust 1.92 / egui 0.33.3; Glow/wgpu candidates, separate web/remote WASM roots; keyboard Tab/arrows/Home/End and named controls.
- Table: 100k virtual rows, bounded worker/artwork; multi-track/album-block moves with cancellation and stale-view rejection; 6000 CPU-only concurrent scrolling/sort/filter/group/playback/monitor samples, not presented frames.
- Media: Unix mpv plus cross-checked Windows named-pipe candidate (hosted fixture tests passed; real mpv pending); explicit ffplay/SDL and offline FFmpeg filter/waveform candidates, not a selected production fallback.
- Capture: one-slot off-UI supervisor with generation isolation, suspend/re-enable/retry and real owned-monitor checks; no default host capture.
- Browser: real disposable Navidrome FLAC/MP3 on three lab engines; six session callbacks, cancellable/deadlined play, lifecycle/reload checks; Firefox/WebKit seeks remain open.
- Packaging/accessibility: rebuilt Ubuntu x86_64 AppImage runs without checkout/Rust; nine-step Orca/eSpeak NG selection journey passed; no physical/device certification.
- Hosted 58814c1: all seven jobs passed, including Windows pipe tests and both AppImage CPUs; P01.07 CI establishment verified.
- Latest checks: 33 default Rust, 56 Python with reference; five external native probes via wrapper passed on Ubuntu x86_64; format/default-WASM-wgpu Clippy and release web builds passed; older runtime manifests stay historical.
- Records: [matrix](p01/README.md), [Orca/CI](p01/screen-reader.md), [Windows IPC](p01/windows-ipc.md), [migration](migration/p01-map.csv), [licenses](licenses/README.md).
- Delivered: polite table status fixes silent Orca selection; nine checks passed; descendant cleanup regression fixed and both independent reviews satisfied.
- Open: calibrated physical budgets, renderer/input/IME/RTL/touch/screen-reader/OS matrix, Windows runtime/OS controls/capture, full native fallback and release licensing; no P01 exit.
- Next: continue open P01 feasibility gates; P02 only after verified P01 exit.
