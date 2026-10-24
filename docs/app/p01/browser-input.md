# P01.02/P01.06 browser input feasibility

This synthetic journey evaluates the full-web and remote WASM composition roots independently of playback. It does not establish P01 exit, mobile/device support, browser screen-reader navigation, physical mixed DPI, OS IME integration or script rendering coverage.

The browser wrapper now mirrors the egui table's row count, selected count and focused synthetic record into a visible, polite DOM status region. It updates text only when the summary changes. This is a browser accessibility boundary for the canvas presentation, not a text setter or private-state test export. It does not expose filter/CSS text, credentials or media data. Canvas widgets still lack a complete semantic DOM tree; a status region alone does not establish accessibility compliance.

The [input harness](../../../tests/browser/input.cjs) uses screenshot-derived CSS coordinates relative to the canvas for the filter (180,54), first row (160,225) and focus-table control (150,142) at 1100 CSS pixels/default scale. It enters synthetic Unicode through Playwright's keyboard text insertion and the focused eframe text agent. It checks actual filtered row counts, exact-title filtering and clear, text-arrow isolation, pointer/touch row selection, focus-control Home/End, and continued keyboard input after resizing to 900x800. It never calls a DOM input value setter or a Rust state setter. The full-web fixture has 1000 rows; the remote has 100. No media source is loaded.

Unicode insertion demonstrates delivery into egui's filter. It does not demonstrate IME preedit/commit/cancel from an operating system. Default fonts visibly lack glyphs for parts of the synthetic Japanese/Arabic text in diagnostic screenshots; filtering does not certify shaping, RTL reading order or font coverage. The native composition-event regression is recorded separately.

## Runtime results and limitations

The final [runtime record](evidence/browser-input-final.json) records each engine/context/root, browser version, source and bundle SHA-256 digests, observed selection/focus summaries and backing-size measurements. Chromium and WebKit run headless; Firefox runs headed under Xvfb for software WebGL. Playwright is externally installed in the isolated Linux lab; this is not Safari, Chrome/Firefox product, previous-stable or physical touch-device certification.

DPR1 pointer and emulated touch contexts exercise both roots in Chromium, Firefox and WebKit. The additional DPR2 contexts are separate diagnostics. Chromium/Firefox's simulated `devicePixelRatio=2` reports a 1100-pixel `ResizeObserver.devicePixelContentBoxSize` and canvas backing width for a 1100 CSS-pixel canvas. eframe 0.33.3 `web/events.rs::get_display_size` prefers that device-pixel box without a further DPR multiplier. The resulting enlarged CSS presentation invalidates the DPR1 coordinate map. These contexts are recorded as limited, with no DPR2 input pass claim. WebKit lacks that device-pixel box in this lab and uses the content-box/DPR fallback; its backing width is 2200 and its DPR2 journey runs. This evidence concerns emulation behavior, not a physical DPI fix or a guarantee across devices.

The [development snapshot](evidence/browser-input-before.json) retains failed provisional outcomes, local screenshot hashes and the original coordinates. It exposed two harness errors: assuming DPR1 coordinates in the enlarged emulated presentation, and an unanchored zero-row matcher that also matched `1000 rows`. The final matcher has a regression check; the final matrix separates DPR1 touch from the DPR2 diagnostic. No application touch behavior was changed to make these tests pass.

A subsequent [shared-lab attempt](evidence/browser-input-lab-exhaustion.json) retained Firefox startup errors after Chromium/WebKit passed. The shared container had 853 of 1024 allowed process/thread entries and many historical defunct browser/Xvfb children. Resource exhaustion was suspected, not proven from the sanitized error class. The final matrix was rerun in a fresh task-owned container with an init process to reap descendants; the shared lab was not restarted.

## Reproduce

Build fresh tracked sources with the pinned Rust/wasm-bindgen setup in the [P01 guide](README.md). Use a new output directory; the harness refuses to overwrite an existing experiment:

```sh
python3 tools/app/build-web.py --out /absolute/new-web-bundles
P01_PLAYWRIGHT=/absolute/playwright P01_SOURCE_ROOT="$PWD" xvfb-run -a node tests/browser/input.cjs /absolute/new-web-bundles /absolute/new-input-results
python3 -m unittest discover -s tests/tooling -p 'test_p01_input_harness.py'
node --check tests/browser/input.cjs
```

`P01_SOURCE_ROOT` must contain the named source/provenance inputs; configured missing inputs fail rather than silently disappearing from the manifest. Run only against owned synthetic bundles, with no profiles or credentials. The harness serves files on an ephemeral loopback port, uses bounded launches/actions/cleanup, records failures by stage and error class, attempts browser cleanup even if context cleanup fails, and closes its server. The source hashes identify relevant input/build files; bundle hashes identify the actual executed WASM. This is a controlled input prototype, not a full source-reproducibility attestation.

The tooling tests cover static-file path rejection, exact row-count matching, evidence overwrite protection, launch-failure records/payload redaction and cleanup after a context-close rejection. Real browser results are separate from these tooling checks. Media/audio lifecycle checks were not rerun because this change does not modify media behavior.
