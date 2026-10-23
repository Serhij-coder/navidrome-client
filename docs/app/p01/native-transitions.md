# P01.05 live native transition energy probe

This continuation measures the managed Rust mpv prototype playing two owned, two-second tones into an isolated PulseAudio null sink. The [optional probe](../../../tests/native-spike/transitions.rs) uses the actual `Mpv::load_local`, append, pause and shutdown paths. FFmpeg records only the explicitly selected owned monitor. No host output, microphone, desktop source, music profile or network stream is used.

Three repetitions of WAV, FLAC and MP3 pairs are collected with playlist prefetch off and on, followed by one real 150 ms pause positive control: 19 captures total. The two playlist occurrences must both be observed as playing. The raw capture holds at most 12 seconds of 48 kHz mono float PCM; samples are discarded after each case, while only numeric metrics/settings are saved. Unexpected EOF, capacity exhaustion, missing/non-finite input and missing source audio fail rather than yielding a zero-gap result. Child capture is killed/reaped before joining its reader.

The detector reports 1 ms RMS windows below 0.02 full-scale between the first and last active windows. This is the longest **captured low-energy run**, anywhere in the signal, not a sample-exact transition gap. Capture timestamps, sample conservation, discontinuities, device latency and human audibility are not certified. Captured spans can differ from the ideal four seconds; no cause is inferred. A short internal sample loss or discontinuity need not contain a complete low-energy window. Three samples per configuration do not establish percentiles or an acceptance budget. The pause control checks that real silence is observable, not that command-to-device timing equals exactly 150 ms.

The [raw record](evidence/live-native-transitions.json) and [manifest](evidence/native-transition-manifest.json) identify the final test source, native change, runtime and AppImage. These exploratory measurements do not close P01.05: crossfade, authenticated streams, mixed format/rate/channel changes, full DSP/waveform/device behavior, production fallback, OS media controls and Windows/macOS runtime remain open. No engine/fallback or production prefetch decision is selected by this probe.

Observed final results: all 18 ordinary captures had zero complete low-energy windows inside the detected signal. Their captured spans ranged from 3952 to 4000 ms against the ideal 4000 ms. The pause control produced a 151 ms quiet run and 4149 ms span; this includes command/sink/capture behavior and is not a calibrated 150 ms device latency measurement. mpv reported `gapless-audio=weak`, default prefetch false, and script loading false. These local captures do not distinguish a preferred production prefetch strategy.

Upstream `src/main/features/core/player/index.ts:104:32` supplies `--load-scripts=no` and defaults playlist prefetch to yes unless explicitly overridden. The native prototype had omitted the script flag; it now matches that default. Actual mpv property reads verify script loading is disabled. Prefetch is changed only by the private test to compare configurations; production settings/extra-parameter normalization remain deferred. The test file is included through the owning crate's Linux `cfg(test)` module, without widening production visibility. Its four default checks cover known silence/padding, invalid input/capacity, premature capture EOF, and deadline/reaping of a stalled fixture encoder. Encoding each owned fixture has a ten-second deadline. The live experiment is explicitly ignored by the normal suite and the existing five-probe CI wrapper.

Reproduce only in an isolated Linux environment with the [guide's prerequisites](README.md). In terminal 1 start a private owned server, with no default device script:

```sh
p01_audio_dir=$(mktemp -d /tmp/p01-transition-audio.XXXXXX)
printf 'PULSE_SERVER=unix:%s/pulse.sock\n' "$p01_audio_dir"
pulseaudio -n --daemonize=no --use-pid-file=no --exit-idle-time=-1 \
  --load='module-null-sink sink_name=p01_transition rate=48000 channels=1' \
  --load="module-native-protocol-unix socket=$p01_audio_dir/pulse.sock auth-anonymous=1"
```

In terminal 2 use the printed socket path and a new evidence path:

```sh
PULSE_SERVER=unix:/path/from/terminal1/pulse.sock \
PULSE_SINK=p01_transition P01_CAPTURE_MONITOR=p01_transition.monitor \
P01_TRANSITION_OUTPUT=/tmp/p01-new-transitions.json \
cargo test -p feishin-native-spike --lib --locked live_current_next_monitor_energy_measurements -- --ignored
```

The evidence path refuses overwrite. Stop terminal 1 with Ctrl+C after the experiment and remove its empty temporary directory. The saved run used these same sink/server arguments, with private configuration, readiness and process-group cleanup managed by the disposable launcher.

The application change also required a fresh native release/AppImage. Reusing a target cache across copied source trees initially returned a stale release without the new script flag. That artifact is excluded. A targeted native package release clean and rebuild produced the verified binary before repackaging. This is a local build-cache correction, not a new distribution guarantee; clean runtime startup/tone/null-mpv results and artifact digests are recorded separately from physical/device and complete licensing certification.

Verification: 37 default Rust tests passed, including all four capture-analysis/failure regressions; six external native probes and two performance probes were ignored by default. The existing five external native probes passed separately after a native dev-cache clean, and the 19-capture experiment passed again after the premature-EOF detection and bounded-encoder fixes. Formatting, default all-target Clippy and Windows x64 MSVC native all-target Clippy passed; the latter is compile-only. The final AppImage passed isolated startup, generated-tone and null-mpv checks without a checkout/Rust. Its packaged executable retains the script flag and has the same ELF build ID as the rebuilt release; packaging adjusts ELF metadata, so both binary digests are recorded. The bundled GPL text matches the root license. No new dependency or complete runtime/native-library notice audit is claimed. All 59 Python tests with frozen optional reference inputs, inventory/seven local reference regressions, node syntax and documentation/whitespace checks passed. Both independent reviewers found no actionable findings, independently passed the four default probe regressions and saved-evidence test, and verified final source/evidence/AppImage hashes. Reviewers did not rerun the opt-in live capture.
