# ADR-001: egui desktop direction and P01 acceptance

Status: accepted desktop direction from the maintainer; integration, renderer and feasibility pending. This tracked record carries the local reference ADR forward without making the ignored reference a build input.

Use egui for native desktop performance and usability. Appearance, widgets and layout may change. Preserve features, workflows, accessibility, localization, browser and remote deliverables. The alternative Dioxus/system webview direction is superseded; another desktop toolkit needs a superseding product decision supported by failed-spike evidence. Candidate eframe renderers remain proposals until measured.

Keep domain/reducers portable and free of UI, HTTP, OS and storage code. A single application owner applies typed queue/playback commands from UI, engines, remote and OS integrations. Background work uses bounded workers, deadlines and request/generation IDs. Do not block the UI or clone/scan full libraries each frame. Playback, timers, reporting and remote continue while rendering is idle or hidden.

P01 acceptance is bounded to the following prototypes, after P00 exits:

| Task | Required evidence | Failure handling |
| --- | --- | --- |
| P01.02 | Native release shell plus separate full-web and remote WASM roots; compare renderer options on the declared hardware | Record renderer/driver/build versions and failures; pin only demonstrated choices |
| P01.03 | 10k/100k virtual table, visible rows plus bounded overscan; selection, grouping, keyboard, column resize/reorder, drag/drop, concurrent artwork/sort/filter/playback | Compare every metric in [performance protocol](../baseline/performance.md); no per-frame full scans or unbounded artwork caches |
| P01.04 | Authenticated actual browser playback, autoplay/CORS/media session/codec errors on the browser matrix | Track browser-specific limits independently of desktop |
| P01.05 | Native current/next playback, process teardown/death, generation isolation and feature replacement/fallback in [ADR-003](003-playback-boundaries.md) | Unsupported required engine features remain open gates |
| P01.06 | Keyboard-only navigation, focus restoration, screen reader on each required OS, RTL/IME, mixed DPI, clipboard, context menus, themes/fonts; clean Linux AppImage startup/assets/playback | Record exact target/assistive technology and remediation for failures; no generic accessibility pass from compilation |
| P01.08 | One representative preset with bounded native analysis and an authorized OS capture path; denial, engine switch, minimize/disposal and measured cost | Record a bounded bridge in ADR-008 if needed; no implicit webview or assumed JS/WebAudio reuse |

Measure visible, unfocused, minimized and tray-only workloads in release mode. A prototype passes only with saved reproducible evidence and calibrated P00 budgets. No runtime result is present yet. Required untested platforms/capture/accessibility cases remain open; a window alone is insufficient. No bulk screens before these gates.

Native theme tokens, fonts, scale and layouts replace DOM/CSS presentation. Preserve imported CSS inertly, report unsupported customizations and provide replacement controls for workflow-relevant settings. Shared egui WASM is an experiment; if it fails browser/touch workflows, record a separate presentation while retaining shared application contracts and both browser deliverables.

P01 update (2026-10-22): the maintainer explicitly authorized workspace/feasibility work ahead of P00 exit. The [task matrix](../p01/README.md) and [verification record](../p01/verification.md) contain actual builds/probes and open acceptance cases. P00 calibration and the platform/accessibility/native-fallback gates are not waived. Glow remains a provisional development default; software-rendered launches do not select a production renderer or certify a platform. Unix mpv, owned Linux monitor capture and browser fixtures establish bounded feasibility only; Windows named-pipe/OS media controls and native fallback remain unresolved. [ADR-008](008-visualization-feasibility.md) records the analysis/preset boundary.
