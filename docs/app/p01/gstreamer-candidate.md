# P01.05 isolated GStreamer candidate

The Linux-only `gst-probe` feature evaluates GStreamer playbin as an independent native playback candidate. It is opt-in development code, separate from the default desktop/WASM build and development AppImage. GStreamer is not selected as the production fallback. The [comparison](evidence/gstreamer-comparison.json) passed 12 owned-tone cases: GStreamer and mpv, WAV/FLAC/MP3, local files and authenticated loopback HTTP. These are decode/control observations with null output, not physical audio or full music-server compatibility.

## Process and source boundaries

The [helper](../../../crates/native-spike/src/bin/p01-gst-peer.rs) uses safe gstreamer-rs 0.24.4 bindings and an isolated child process. It speaks a subset of the existing probe JSON IPC protocol so the managed [Mpv](../../../crates/native-spike/src/mpv.rs) transport can correlate commands and own process/socket cleanup. The helper identifies itself as GStreamer in its version response; it is not mpv. There is one connection, one pipeline owner, bounded 64 KiB input lines and generic errors without native payloads. No queue, decoder, FFI or general scripting runtime was added.

The helper requires `--ao=null` and supplies explicit synchronized audio/video fakesinks. It rejects system/Pulse output and unknown launch flags before binding its socket. It supports source replacement, pause/resume, observed position/duration, absolute accurate seek, volume and EOS queries. Append, filters, device selection and other IPC commands fail explicitly; the existing parent retires an endpoint after a rejected command. Unsupported operations do not become successful no-ops. This is a small compatibility protocol for evaluation, not a production capability contract.

All native bus messages are dropped synchronously after retaining only error/EOS atomic flags, so streaming threads cannot accumulate a message queue. Pipeline transitions wait up to two seconds each; the parent's existing three-second IPC deadline and graceful/forced process cleanup contain stalled native calls. Multiple transitions can exceed that parent budget and fail. Native library calls, kernel scheduling and kill/reap do not acquire a hard realtime guarantee. The helper's blocking work never runs on the UI or an async executor.

`Mpv::start_loopback_probe` selects null output and clears child HTTP/HTTPS/all-proxy environment variables, setting child no-proxy to `*`; the normal `Mpv::start` path is unchanged. [URL validation](../../../crates/native-spike/src/probe_source.rs) admits only explicit numeric `http://127.0.0.1:PORT` inputs of at most 4096 bytes, without userinfo, fragments, raw whitespace or controls. Query credentials are opaque. The GStreamer source disables HTTP redirects and configures a two-second I/O timeout. The initial URL validator cannot constrain mpv's redirect policy, so the mpv comparison uses only the trusted owned server and never sends it a redirect. This is not a general streaming URL/authentication policy or a substitute for required LAN server support.

An empty GStreamer proxy property clears the explicit proxy setting; libsoup can still use its default system resolver. Child environment clearing/no-proxy settings were used in the owned container, but comprehensive desktop/system proxy bypass was not verified. These fixtures carry disposable tokens and do not accept production credentials or load the user's desktop configuration.

The [test fixture](../../../tests/native-spike/gst_probe.rs) binds an ephemeral loopback port, requires a per-run query token, serves only its owned in-memory routes, implements byte ranges, limits each media payload to 2 MiB and headers to 8192 bytes, and handles one connection at a time with deadlines. It spawns no per-request tasks. Redirect rejection is verified for GStreamer only. Tokens, request bodies, stream URLs and native error payloads are absent from saved evidence.

## Observed results and limits

Six default candidate/fixture tests passed. They cover real GStreamer WAV pause/play, seek arrival, source replacement preserving pause, volume, EOS, process death and PID/socket cleanup; explicit candidate selection after a missing primary; unsupported operations; token/range/method/route failures; malformed media; redirect denial; malformed JSON/request IDs and oversized IPC; and refusal of system output. Two library tests cover initial URL validation. Test teardown waits for child cleanup even after an assertion panic.

The external comparison passed all 12 cases with GStreamer 1.24.2, mpv 0.37.0 and FFmpeg 6.1.1 on the owned Ubuntu 24.04 x86_64 lab. Each case pauses before loading, observes initial position, seeks once to 1.25 seconds and observes arrival, resumes until position advances, then verifies pause stability. The HTTP server observed six range requests. It is a loopback query-auth fixture, not a live Navidrome/Subsonic/Jellyfin authentication or TLS test.

Timing fields measure distinct call boundaries. `load_ack_ms` includes the load call; GStreamer currently waits for preroll in that call, while mpv can acknowledge before readiness. `ready_ms` begins after that call returns. `seek_ms` includes acknowledgement plus observed arrival within 0.1 seconds. Integer millisecond zeros are a reporting floor. These single samples are not a calibrated startup/seek benchmark, a preferred engine result, a percentile or a device latency measurement. No CPU/RSS/GPU or physical-output budget is established.

The missing-primary test explicitly starts the candidate after a failed executable launch. It does not implement an automatic fallback policy, migrate a live queue or restore a source/position between engines. The [serialized handoff](native-handoff.md) and this candidate remain separate experiments. Production fallback must still coordinate one owner, engine capabilities, generation/request identities, source/authentication lifetime, saved position/status, capture/OS consumers and failure recovery. Gapless/crossfade, speed/pitch, DSP/waveform, metadata/radio, real devices and Windows/macOS remain open.

## Reproduction and CI

On Ubuntu 24.04 install the optional build/runtime packages in a disposable environment:

```sh
sudo apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-libav
cargo clippy -p feishin-native-spike --features gst-probe --all-targets --locked -- -D warnings
python3 tools/app/test-native.py --gstreamer-probe
python3 tools/app/test-native.py --gstreamer-codecs
```

The last mode also needs external mpv and FFmpeg. Set `P01_GST_OUTPUT` to a new path to save numeric comparison results; overwrite is refused. `P01_MPV` may identify a trusted mpv executable. The default Cargo workspace tests do not enable this feature. The Linux native CI job builds/lints and runs candidate/library fixtures. Both AppImage architecture jobs additionally run the external codec comparison before packaging. These new hosted results must be observed separately; local x86_64 execution does not certify aarch64 or the other desktop platforms. No artifacts are uploaded or released.

## Dependencies and notices

The lockfile adds 24 packages and retains every previously locked package version. Optional Linux gstreamer 0.24.4 has declared Rust MSRV 1.83; this workspace still pins Rust 1.92.0. The explicit URL parser reuses locked url 2.5.8. No native GStreamer dependency enters portable core, WASM or the default desktop build. Non-Linux probe targets are compile-only stubs, not supported playback implementations.

The [notice collector](../../../tools/app/notices.py) now accepts an explicit feature scope:

```sh
python3 tools/app/notices.py --features feishin-native-spike/gst-probe /tmp/new-gst-notices
```

This copied 395 registry notice sets for the selected feature/platform graph, including all 24 new packages, using exact published license/manifests/readmes. The default graph remains 371 sets. Feature selection is significant: an exploratory `--all-features` metadata collection also activated a pre-existing optional dependency, block 0.1.6, whose package lacks standalone license evidence; that broader collection failed closed. This increment does not resolve or claim that broader optional-feature notice gap.

GStreamer native libraries/plugins are external lab prerequisites. The default AppImage does not bundle the helper or GStreamer. Rust binding licenses do not establish licenses/source obligations for the native core, plugins, codecs or their dependencies. [Native candidate provenance](../licenses/gstreamer-candidate.json) records inspected package versions and notice hashes; a complete plugin/runtime closure, corresponding-source review and clean packaged target matrix are required before distribution or adoption. See the [increment manifest](evidence/gstreamer-manifest.json) for final checks, review outcomes and limitations. No P01 exit is claimed.

Both independent reviewers found no actionable findings. The operations reviewer independently reran the library/candidate tests and all 12 external comparison cases. Exact independent checks are in the increment manifest; the new hosted revision and remaining platform/device gates are separate.

Hosted follow-up: [f4b76ac run 40723301849](evidence/hosted-gstreamer-candidate.json) passed all seven jobs, including the optional Linux candidate checks, codec comparisons on x86_64/aarch64 and both default AppImage smoke jobs. Windows/macOS jobs validate the default native path, not GStreamer playback there. This is additional build/control/packaging evidence; production fallback, physical output, calibrated performance and native distribution licensing remain open.
