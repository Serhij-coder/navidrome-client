# Project scratchpad

- Stage: P01.01/P01.07 complete; remaining P01 tasks have prototypes/evidence but open gates. P02-P14 have not started; this is not a usable music client.
- Delivered: [prepared local handoff](p01/prepared-handoff.md) checks and both reviews passed; 8 fixtures plus real mpv/cross-engine probes. New hosted result remains separate.
- Sequencing: [P01 audit and P02-start proposal](p01/exit-decision.md) pending maintainer decision. Existing P01-first rule remains in force; no gate waived.
- Native UI/table: Rust1.92/egui0.33.3, Glow/wgpu candidates; 100k virtual rows, keyboard/drag/grouping, bounded worker/artwork. Physical renderer and calibrated cost gates remain open.
- Media: bounded command owner, serialized replacement, private-bus MPRIS subset and optional GStreamer candidate. Prepared path verifies explicit local position/pause/volume; production fallback/source/auth/consumer policy remains open.
- Browser/remote: separate WASM roots; lab Navidrome codecs, Media Session and input/lifecycle checks. Device/browser/auth policy and Firefox/WebKit boundary limitations remain.
- Capture/visualizer: bounded original spectrum/waveform and opt-in owned Linux monitor with disposal/retry. Actual preset, cost, OS permission and Windows/macOS evidence remain open.
- Packaging/input: both AppImage CPUs have hosted smoke; local clean x64 artifact/clipboard/Orca and toolkit IME/scaling checks passed. Physical input/font/RTL/platform and native distribution licensing remain open.
- Hosted f4b76ac: all seven jobs passed, including optional GST and both AppImage CPU codec/smoke checks; see [result](p01/evidence/hosted-gstreamer-candidate.json). Prepared increment not yet hosted.
- Current checks: 86 workspace tests/13 ignored; final prepared target8 pass; real mpv4 and mixed-engine3 replacements pass; MPRIS7 pass; Python71 pass/8 optional skips; fmt/default/GST/MPRIS/Windows Clippy pass.
- Limits: prior96a8a6d CI failure and one real MPRIS seek deadline remain unexplained; passing repetitions do not resolve their cause. Source/record links retain historical evidence.
- Records: [P01 matrix](p01/README.md), [migration](migration/p01-map.csv), [input/Orca](p01/screen-reader.md), [playback ADR](adr/003-playback-boundaries.md), [licenses](licenses/README.md).
- Next: resolve sequencing decision. Prefer portable P02 foundations then a partial Navidrome vertical slice if authorized; no more independent P01 expansion without a concrete blocker.
- Workflow: Sol high orchestration, Terra medium implementation, Sol medium escalation, Luna high research and one combined Sol medium review; compact contexts/batched delivery. See [setup](development-workflow.md).
- Workflow checks (2026-10-28): strict config/read, five TOMLs/role assertions, tracked links, inventory, seven local reference tests and Node syntax passed; app-task cost/quality pilot remains unmeasured.
