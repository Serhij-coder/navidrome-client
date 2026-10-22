# P01 continuation: input, media and lifecycle probes

This increment continues P01.02-P01.08 from workspace commit `8b987c6`. It is not P01 exit or a production feature port. No P02 work is included. The [initial record](verification.md) and its hashes describe the earlier snapshot; [continuation evidence](evidence/continuation.json) identifies this increment. All unverified target requirements remain in the [task matrix](README.md).

## Changes and observed results

| Task | New implementation/evidence | Remaining gate |
| --- | --- | --- |
| P01.02 | Rebuilt native Glow/wgpu and separate web/remote roots; software renderer smoke; six cross-target checks; initial hosted Linux/Windows/WASM results | Physical GPU/driver comparison, native target launches, browser/mobile input matrix and production renderer choice |
| P01.03 | Mouse-free Tab entry into the table; arrows/Home/End/range selection; bounded virtual rendering after boundary jumps; 6000 CPU layout samples with actual worker sorting/filtering/grouping, PNG churn, owned mpv playback and monitor analysis | Presented frames, physical input latency, total helper RSS, calibrated budgets, real network/library workloads, multi-record/group drag and minimized/tray resource measurements |
| P01.04 | Cancellable, generation-checked play operations with an eight-second deadline; six Media Session actions, static safe metadata and position state; explicit reload after errors; actual disposable Navidrome FLAC/MP3 on all three lab engines | Reported seeking failures, real OS media-key activation, other servers/transcodes/cookies and current/previous Safari/mobile matrix |
| P01.05 | Explicit ffplay/SDL alternative probe with initial seek, process ownership/generation/death checks; missing-mpv then alternative decode test; FFmpeg concat/crossfade/lowpass/waveform evaluation | ffplay lacks control IPC and is not selected as production fallback; dynamic pause/seek/device/stream handoff, Windows IPC/OS controls and live gapless/crossfade timing remain unresolved |
| P01.06 | Keyboard-only journey and explicit filter/CSS/scale names; isolated Linux AT-SPI tree audit; rebuilt Ubuntu x86_64 AppImage | Orca/screen-reader journeys, IME/RTL/fonts, DPI/clipboard/touch and physical OS matrix; current aarch64 artifact/runtime and release licensing |
| P01.07 | Initial hosted x86_64/aarch64 AppImage jobs and Linux/Windows/WASM checks passed; macOS tests failed; native jobs now pin Python 3.11 and annotate failed Rust test identifiers without assertion payloads | Cause of initial macOS failure needs authenticated logs or the new public test-name annotation; current revision hosted results are separate |
| P01.08 | Bounded capture supervisor handles suspend/re-enable/retry on a worker; one-slot frames and generations reject stale work; real owned monitor resumes while mpv continues; browser disable/visibility/pause/shutdown probes | Actual permission prompts, Windows/macOS capture, actual presets/bridge decision and physical frame/CPU/GPU/total-memory measurements |

## Native capture and alternative engine

`AnalysisSession` owns a supervisor thread and a single capture process/PCM reader. UI updates change the desired active state without launching, stopping or joining a child. State changes clear the one-slot frame immediately; the supervisor rejects old generations. Hidden/minimized/disabled consumers dispose capture on the worker. Re-enable reuses only the explicitly configured source from this session. Failures latch until off/on or explicit retry; there is no repeated process spawn loop. No source is selected automatically. Final shutdown joins owned work outside normal update callbacks.

Tests use both a deterministic disposable PCM-producing process and real FFmpeg/mpv on the isolated `p01_owned.monitor`. They check rapid toggles, cleared stale frames, failure retry, bounded shutdown and child reaping. The real test suspends/re-enables capture twice and verifies mpv position continues. None of this records a host microphone or desktop source.

`FfplayProbe` is an existing native decoder/SDL candidate, not a new decoder and not the selected engine. Its file input, initial timestamp and child lifetime are bounded/validated; SDL's dummy driver proves decode and completion, not physical sound. ffplay exposes no JSON command interface, so this candidate alone does not satisfy pause/resume, dynamic seeking, device selection or authenticated streaming handoff. The default remains the provisional mpv experiment. Run the explicit alternative probe with a trusted executable and owned file:

```sh
cargo run -p feishin-desktop -- --ffplay-probe-null /usr/bin/ffplay /tmp/p01-owned.wav
python3 tools/app/probe-native-media.py --ffmpeg /usr/bin/ffmpeg --samples 30 --out /tmp/p01-new-native-filters.json
```

[Offline filter samples](evidence/native-filters.json) have 30 runs for concat plus 12 speed/fade/curve combinations: 0.5x/1x/2x, 1 s/5 s, linear/quarter-sine. Concat sample count matches the ideal; `atempo` produces approximately -44.5 ms / +7.65 ms duration differences at 0.5x / 2x. These are whole-output duration differences, not measurements of transition timing or upstream fade-style equivalence. A lowpass decode supplies at most 256 waveform envelope bins. The experiment invokes installed FFmpeg on generated local tones only. It does not implement a live fallback, PCM sink or waveform UI/cache.

## Concurrent and accessibility probes

The [concurrent samples](evidence/concurrent.json) use non-monotonic synthetic titles and non-contiguous album metadata, the actual latest-only worker, the owned PNG decode/cache and actual owned-tone mpv/monitor processes. The probe waits for a nonzero source frame before sampling; its first attempt finished too quickly to observe monitor startup and was corrected. It saves 6000 CPU layout calls, 20 observed view completions, monitor-frame counts and media positions. Owner-only Linux peak RSS excludes children and is not the total application budget. Frame input/time is synthetic, with no presentation/GPU/physical latency claim; background load, governor and thermals are uncontrolled.

```sh
P01_CAPTURE_MONITOR=p01_owned.monitor P01_CONCURRENT_OUTPUT=/tmp/p01-new-concurrent.json cargo test -p feishin-spike-ui --release --lib concurrent_probe --locked -- --ignored
```

This requires the same separately provisioned owned PulseAudio server and `P01_MPV` described in the [runbook](README.md); do not select a default host monitor.

The [AT-SPI tree evidence](evidence/accessibility.json) and optional [AT-SPI audit](../../../tests/desktop/accessibility.py) runs against an AppImage in an isolated Ubuntu D-Bus/Xvfb session with `at-spi2-core`, `python3-pyatspi` and `dbus-x11`. Its first tree exposed an unnamed filter entry; explicit `labelled_by` now associates the filter and inert CSS inputs with names, and the scale slider has a name. The audit checks the actual text-entry name, table focus button and analysis control, with a 512-node/depth bound. It is not an Orca speech/focus journey or certification for all virtual rows/platforms.

```sh
dbus-run-session -- xvfb-run -a /usr/bin/python3 tests/desktop/accessibility.py /absolute/path/Feishin-Rust-P01-x86_64.AppImage /tmp/p01-new-accessibility.json
```

## Browser evidence and limits

The [saved browser results](../../../tests/fixtures/p01/browser/2026-10-22-continuation/browser-results.json) retain Playwright 1.56.1's Chromium 141.0.7390.37, Firefox 142.0.1 and Linux WebKit 26.0. Each engine ran synthetic token rejection/WAV/CORS/codec cases, nonzero analysis, disable/re-enable, synthetic visibility notification, pause, shutdown, stale play failure and timeout. Optional Media Session APIs were made to throw in a separate page; visible playback still worked. Calling captured Media Session handlers is callback integration evidence, not a trusted OS key event. The metadata contains a fixed fixture title, never a URL. Recovery also clears the reload latch when replacing a failed source; a real-browser regression verifies subsequent pause/resume retains position.

The fixture now supports HTTP byte ranges and preflight, required to exercise seeking. Chromium reached 2 s and both bounds. Firefox reached 2 s/0 s but raised media error 3 on exact-duration WAV seek; visible Load/Play reloaded the source and recovered. Lab WebKit reported a completed seek at 0 s instead of the requested 2 s. The shell verifies the completed position and reports failure rather than claiming success. These observed lab limits remain open, without inferring a cause or asserting that a different stream/device behaves identically.

For actual server streaming, the existing isolated Navidrome 0.61.0 fixture served its two owned FLAC/MP3 tones directly to each browser. Invalid token requests failed, valid playback advanced and WebAudio was nonzero. Credentials/authenticated URLs stayed in a private 0600 file, tab memory and the network request; no server URL, token, password, request header or server response was saved. Screenshots are taken before the private server cases. No proxy was added. Other servers/transcodes/cookies and physical output remain untested.

Optional `P01_SERVER_STREAMS` points to a private JSON list of `{fixture, format, validUrl, invalidUrl}`. This experiment accepts only `Navidrome <numeric version>`, `flac`/`mp3` and HTTP loopback `127.0.0.1:14533/rest/stream`; it is not a generic service/proxy input. Parser failures are sanitized and regression tested. Omit the variable for the self-contained synthetic experiment.

## CI and verification

[Hosted initial results](evidence/hosted-initial.json) refer only to commit `8b987c6`: six jobs succeeded, while macOS `cargo test` exited 101. The public job/annotation API provides that status; detailed log access returned HTTP 403, so the failure cause is unknown. Pinning the documented Python prerequisite is reproducibility work, not a claimed diagnosis/fix. The new wrapper streams normal Cargo output and emits only failed test identifiers as public CI annotations; tests check that arbitrary assertion/credential-like text is excluded. No artifact upload/publication was added.

The final local set includes 23 default Rust tests (five external native and two performance tests ignored), five explicit external native tests, the concurrent release experiment, 47 Python tests with the optional frozen reference environment, default/WASM/wgpu Clippy, formatting and six cross-target compile checks. Separate software Glow/wgpu/hidden launches, clean copied-input builds/tests and the rebuilt x86_64 AppImage are checked independently. These checks do not turn the open acceptance cases into passes. The dependency change adds pinned browser-only futures cancellation/timeout support; generated notice inventories include its exact registry texts.

Both independent reviewers inspected the implementation, tests, architecture and evidence. The correctness reviewer reproduced an unchecked long-TMPDIR Unix socket failure. The fix checks exact encoding/path length and falls back to a unique short directory with explicit mode 0700. The new regression initially exposed tempfile's default 0755 directory mode, which was also fixed and rerun. Normal and long-root tests now verify privacy and cleanup. This does not diagnose the original macOS failure.

Primary review also replaced a racy wake-counter assertion with an actual failed-child launch count, proving no automatic retry and explicit retry/off-on behavior without assuming notification order. Both reviewers accepted these fixes after independent focused reruns. The 23-test workspace, five real native tests, concurrent probe and rebuilt AppImage were rerun after the native fix; exact final source/evidence hashes are in the manifest. The AT-SPI audit emitted an unsupported-cache-object warning while its bounded tree/name checks passed; screen-reader behavior remains unverified. Final evidence review and delivery are recorded in the manifest; no P01 exit is claimed.
