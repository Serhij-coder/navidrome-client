# Project scratchpad

- Stage: P01.01 complete; P01.02-.08 have prototypes/evidence but open exit gates. No P02+ implementation.
- Native UI: pinned Rust 1.92 / egui 0.33.3; Glow/wgpu candidates, separate web/remote WASM roots; keyboard Tab/arrows/Home/End and named controls.
- Table: 100k virtual rows, bounded worker/artwork; 6000 CPU-only concurrent scrolling/sort/filter/group/playback/monitor samples, not presented frames.
- Media: Unix mpv; explicit ffplay/SDL and offline FFmpeg filter/waveform candidates, not a selected production fallback.
- Capture: one-slot off-UI supervisor with generation isolation, suspend/re-enable/retry and real owned-monitor checks; no default host capture.
- Browser: real disposable Navidrome FLAC/MP3 on three lab engines; six session callbacks, cancellable/deadlined play, lifecycle/reload checks; Firefox/WebKit seeks remain open.
- Packaging/accessibility: rebuilt Ubuntu x86_64 AppImage runs without checkout/Rust; Linux AT-SPI names audited after fixing unnamed filter; no screen-reader/device certification.
- Hosted initial CI: Linux/Windows/WASM and both AppImage CPUs passed; macOS tests failed, detailed logs unavailable. New native annotations expose failed test names.
- Checks: 23 default Rust + five explicit native runtime; 47 Python with reference; format/Clippy/WASM, six cross checks, browser and copied-input checks passed.
- Records: [continuation](p01/continuation.md), [matrix](p01/README.md), [migration map](migration/p01-map.csv), [license gaps](licenses/README.md).
- Review: both independent reviewers accepted final source/tests/evidence after socket/privacy and retry-regression fixes.
- Open: calibrated physical budgets, renderer/input/IME/RTL/touch/screen-reader/OS matrix, Windows IPC, full native fallback and release licensing; no P01 exit.
- Next: inspect current hosted results and remaining P01 gates; P02 authorized only after verified P01 exit.
