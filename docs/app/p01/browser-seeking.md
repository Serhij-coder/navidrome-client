# P01.04 non-looping browser seeks and fixture correction

The shell's audio element defaults to non-looping playback. The earlier browser journey forced `audio.loop = true` to keep its five-second tone alive through analysis checks. A controlled comparison now isolates that setting: Linux WebKit 26.0 reaches 2 s with looping off, but reports a completed seek at 0 s with looping on. This narrows the earlier WebKit observation; it does not establish a general failure of the shell's default interior seeking or explain the browser's internal cause. No application media code changed.

The [browser journey](../../../tests/browser/p01-browser.cjs) now asserts the non-looping default and uses a 30-second generated tone for lifecycle checks. It installs completion/error listeners before each seek request and records a three-second timeout separately. Polling `seeking` alone could observe a position before WebKit's queued event. Two additional fresh pages compare direct media-element seeks with looping off/on using the original five-second WAV bytes, without Media Session callback dispatch. Sources still pass through the shell's real media/WebAudio boundary. This is a direct-element comparison within the shell, not a separate engine or plain-element page.

[Saved results](../../../tests/fixtures/p01/browser/2026-10-23-seeking/browser-results.json) and the [source/artifact manifest](evidence/browser-seeking.json) bind the final harness to release WASM bundles from `91f4d38`. Earlier results/manifests remain historical. The three-engine experiment completed successfully, with these separate seeking gates:

| Lab engine | Non-looping Media Session callbacks | Original 5 s direct-element comparison | Gate |
| --- | --- | --- | --- |
| Chromium 141.0.7390.37 | 2 s, lower bound, 30 s end and return to 0 s completed | 2 s/end completed in both modes | Passed for this fixture |
| Firefox 142.0.1 | 2 s/lower bound completed; exact 30 s end raised media error 3 | Exact 5 s end raised error 3 in both modes; 2 s completed | Open |
| Linux WebKit 26.0 | 2 s/lower bound/end completed; return from end reached 0 s without a completion event before the deadline | Both targets completed with looping off; with looping on both reported 0 s | Open |

The harness prints `seekingGate` separately from shell completion. Exit zero means the experiment completed and its other assertions passed; it does not convert a recorded open seeking gate into acceptance. Firefox's error and WebKit's missing/mismatched completions remain required remediation, including bounded application-side seek completion and tests against other codecs/servers. No deadline, target or tolerance was relaxed to turn these cases into passes.

The same run exercised nonzero WebAudio analysis, disable/re-enable/synthetic visibility/pause/shutdown, stale-play cancellation/eight-second play deadline, optional API failure, token rejection, CORS and malformed codec handling on all three engines. Real-server fixtures were omitted from this rerun; the earlier Navidrome FLAC/MP3 evidence is historical. Browser media clocks, analyser samples and synthetic callback dispatch do not establish physical sound, OS media keys, mobile/Safari/current/previous support or P01 exit. Screenshots from this audio-focused rerun stay local; no visual-change claim is made.

An early rerun exposed an environment prerequisite: Firefox's media clock advanced while AudioContext stayed suspended because the lab PulseAudio service was absent. Running the unchanged application against an explicitly owned PulseAudio null sink restored nonzero analysis. A functioning isolated audio service is therefore a reproduction prerequisite; a moving media clock alone is insufficient. The final run used no host audio output or capture source.

In a disposable Linux environment with the guide's browser prerequisites, install PulseAudio if absent:

```sh
sudo apt-get install --no-install-recommends pulseaudio
```

In terminal 1, run an owned sink in the foreground. The printed socket path is used in terminal 2; `-n` prevents loading the default device script:

```sh
p01_audio_dir=$(mktemp -d /tmp/p01-browser-audio.XXXXXX)
printf 'PULSE_SERVER=unix:%s/pulse.sock\n' "$p01_audio_dir"
pulseaudio -n --daemonize=no --use-pid-file=no --exit-idle-time=-1 \
  --load='module-null-sink sink_name=p01_browser' \
  --load="module-native-protocol-unix socket=$p01_audio_dir/pulse.sock auth-anonymous=1"
```

In terminal 2, use the printed `PULSE_SERVER` value with the [guide's browser command](README.md). After the experiment, stop terminal 1 with Ctrl+C and remove its empty temporary directory. The saved run used the same foreground/null-sink arguments and a temporary private directory, with process ownership/readiness and final termination managed by the disposable test launcher.

Node syntax, both failure/redaction regressions, the saved-evidence integrity regression, all 57 Python tooling tests with frozen optional reference inputs, tracked documentation/whitespace, inventory and seven local reference regressions passed. The release WASM roots were rebuilt. Both independent reviewers resumed after a temporary account usage limit, inspected the final diff/evidence, independently passed all three focused tooling tests and verified the six bundle hashes. Both reviews are satisfied with no actionable findings. This task changes the experiment and its evidence, not the application's media implementation or supported platform policy.
