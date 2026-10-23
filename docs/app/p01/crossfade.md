# P01.05 dual-mpv crossfade and gain characterization

This experiment uses one test owner for two managed mpv instances. It sends validated percentage-volume commands over their existing bounded IPC boundaries, derives fade progress from the outgoing media position, and records an explicitly owned PulseAudio monitor. It adds no decoder, desktop webview, production queue owner or default crossfade behavior. The normal application remains a feasibility shell.

The portable `FadeCurve` helper implements coefficients from the active Feishin `web-player.tsx` handler and the squared normalized volume mapping from `web-player-engine.tsx` / `player-utils.ts`. The [198-point fixture](../../../tests/fixtures/p01/crossfade/upstream-gains.json) comes from [offline replay](../../../tests/reference/p01-crossfade.cjs) of exact hash-pinned original functions using TypeScript 6.0.3. Four source files are verified before evaluation. PLAYING/transition state and the setter connection are mocked; React scheduling, real browser audio and the entire transition workflow are not executed by this replay. Default Rust tests need only the tracked fixture; the reference package/parser is optional for regeneration.

Two source details matter:

- The active exponential handler inverts an already decreasing outgoing easing value. Its outgoing and incoming coefficients both rise from zero. `UpstreamExponential` records that source behavior; this experiment does not silently repair it or enable it in native playback. A production correction/compatibility decision remains open.
- The engine squares its normalized slider value before assigning media volume. Thus the named equal-power curve assigns 0.5/0.5 gain scalars at its midpoint, not 0.707/0.707. The fixture records both slider coefficients and assigned scalars. Legacy/unknown style values fall back to equal power in this active handler; the older `utils/list-handlers.ts` implementation is not the selected call path.

The native calibration records fixed mpv percentage plateaus. On the tested mpv 0.37 configuration, 50 percent gives approximately one eighth of the full-volume tone amplitude and 25 percent gives approximately one sixty-fourth. The prototype therefore applies the cube root of each desired assigned gain scalar when sending percentage values. This conversion is private to the experiment and checked by the calibration; the public `Mpv::set_volume_percent` API exposes percentages directly, validates finite 0-100 input, and does not claim a universal linear gain law.

The live candidates cover linear, equal-power and S-curve, each using a three-media-second fade at speed 1 with owned 750/1500 Hz WAV inputs. Spectral projections over 100 ms capture windows establish that both tones overlap and that the inactive tone is suppressed at the endpoints. Command traces and spectra are saved separately. The middle window is anchored to received capture frames and is not a synchronized device timestamp. The test checks overlap/headroom and the source-derived control values; it does not claim a fitted full waveform, constant acoustic power, artifact-free sound, exact latency or audible parity. The exponential quirk is source-characterized only.

A stale generation is rejected without changing either current player. Peer death or a failed second gain update stops both players; default regression tests verify this failure cleanup. Volume writes are sequential, not an atomic mixer operation. The owner and scheduler are test-only prototypes; production worker scheduling, pause/seek/repeat/no-next policies, authenticated streams, all fade durations/speeds/formats, native fallback, Windows/macOS output and total helper CPU/RSS remain open. P01.05 and full P01 exit are not claimed.

Run only in the isolated owned sink described in the [native transition guide](native-transitions.md). The optional calibration and dual-player tests require new evidence paths and are ignored by default:

```sh
PULSE_SERVER=unix:/path/to/owned/pulse.sock PULSE_SINK=p01_transition \
P01_CAPTURE_MONITOR=p01_transition.monitor P01_GAIN_OUTPUT=/tmp/p01-new-gain.json \
cargo test -p feishin-native-spike --lib --locked live_mpv_volume_calibration -- --ignored

PULSE_SERVER=unix:/path/to/owned/pulse.sock PULSE_SINK=p01_transition \
P01_CAPTURE_MONITOR=p01_transition.monitor P01_CROSSFADE_OUTPUT=/tmp/p01-new-crossfade.json \
cargo test -p feishin-native-spike --lib --locked live_two_mpv_crossfade -- --ignored
```

Reference replay, when the frozen package and parser are available:

```sh
node tests/reference/p01-crossfade.cjs /path/to/frozen/reference /path/to/typescript.js /tmp/p01-new-source-gains.json
```

Each native capture is bounded to at most 24 seconds of mono 48 kHz PCM; raw PCM is discarded after measurement. Existing transition capture keeps its 12-second limit. No host microphone, desktop source, physical output or real profile is used. No new registry dependency/version is added: serde_json was already locked and is now a model-test dependency for the saved fixture. Attribution remains Feishin/GPL-3.0.

The [manifest](evidence/crossfade-manifest.json) binds source changes and [calibration](evidence/native-volume-calibration.json), [live spectra/control traces](evidence/live-native-crossfade.json), the [198-point replay](../../../tests/fixtures/p01/crossfade/upstream-gains.json), and the repeated [19-capture regression](evidence/crossfade-transition-regression.json). All three live curves passed overlap, endpoint suppression and headroom checks. The model/source, percentage validation, spectral separation, stale generation, peer death and partial-update teardown regressions passed. No new playback behavior is enabled in the desktop/browser/remote shells, and no new AppImage result is claimed for these unused prototype APIs. Older artifact/evidence records remain historical. Final verification passed: format/default all-target Clippy, Windows x64 MSVC native Clippy (compile only), WASM Clippy, 44 default Rust tests, 63 app-tooling tests with frozen optional replay, seven local reference tests, inventory, JavaScript syntax and tracked documentation/whitespace checks. Five existing external native probes passed in the isolated sink. Both independent reviewers found no actionable findings and independently passed the affected default regressions; they did not rerun live captures. No fixes were requested.
