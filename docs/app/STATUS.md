# Project scratchpad

- Stage: P01 workspace/feasibility started at maintainer request; P00 and P01 exit gates remain open. No P02+ implementation.
- Implemented: pinned Rust 1.92 / egui 0.33.3 workspace, native Glow/wgpu candidates, separate web/remote WASM roots.
- Table: 100k native virtual rows, synthetic selection/group/sort/filter/drag and bounded artwork probes; bounded latest-only worker; browser capped at 1k.
- Media: Unix local-file mpv IPC/lifecycle and explicit bounded Linux monitor analysis; no native fallback or Windows IPC yet.
- Browser: disposable token-gated WAV, WebAudio analysis, wrong-token/CORS/codec cases on Chromium/Firefox/WebKit lab engines.
- Packaging: Ubuntu x86_64 AppImage starts in a separate runtime without checkout/Rust; external mpv null-output probe; no physical audio/arm64/release certification.
- Verification and open subphases: [P01 runbook](p01/README.md), [verification](p01/verification.md), [migration map](migration/p01-map.csv).
- Last checks: 17 default Rust + four native runtime tests; 45 Python with reference; Clippy/format/WASM/browser/AppImage passed; both reviews satisfied.
- Required gaps: P00 calibration, real concurrent performance, screen readers/IME/RTL/DPI/touch, native fallback, OS/capture/target matrix.
- Licensing: dependency/font inventory and exact-revision texts added; [native runtime and exact-notice release gates](licenses/README.md) remain open.
- Next: finish only P01 validation/remediation; do not start bulk screens or P02.
