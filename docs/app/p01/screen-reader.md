# P01.06 Orca keyboard and selection announcements

The controlled Orca journey exposed a real table accessibility gap: the keyboard focus stayed on the table entry button while arrow/Home/End selection changed silently. A visible, polite AccessKit status now announces the focused synthetic track, album and selected count. It retains keyboard focus and adds no periodic repaint or full-view scan. Mouse selection requests one follow-up repaint to publish its status. This is native feasibility evidence; P01.06 and full P01 exit remain open.

The [before result](evidence/orca-before.json) ran against the `58814c1` AppImage. Control names, table entry and focus exit/return passed, while all four track/range/boundary announcement steps failed. The [fixed result](evidence/orca-status.json) passed all nine checks. The [manifest](evidence/orca-manifest.json) binds both artifact digests, the exact UI source override, harness and saved results. Earlier manifests remain historical. A new Rust regression checks the emitted status role, polite live setting, stable node ID, focused/range/boundary text, keyboard-focus retention and virtual row bound.

The optional [journey](../../../tests/desktop/orca.py) runs the installed Orca 46.1 CLI and its real AT-SPI event/speech handlers, Speech Dispatcher 0.12.0-rc2 and eSpeak NG 1.51. The only Orca instrumentation configures its debug channel with line buffering before running the packaged CLI. Its ordinary buffered log initially withheld the final Home message until more events arrived; logging instrumentation fixed that false negative without changing the expected message or deadline. No speech or accessibility event is fabricated. Owned service groups receive bounded termination even when their leader exits first. A scoped Linux child subreaper adopts orphaned descendants; cleanup reaps them and verifies each group is gone. A real TERM-ignoring descendant regression verifies this path. Private logs are bounded and deleted; saved output contains only exact known synthetic phrases and check results.

Orca speech autospawn initially failed in this container. The final harness explicitly owns PulseAudio with a null sink, Speech Dispatcher with a private Unix socket, and Orca with private preferences. It clears inherited speech-service addresses and gives activated accessibility/settings services a private profile in the disposable D-Bus session. It opens no host microphone, desktop capture source or physical output device. Keys are synthetic X11 input on Xvfb. The evidence records speech requests, not human evaluation of the spoken audio or real-keyboard latency. Manual development runs also emitted the previously observed AT-SPI Cache GetItems warning; these passing focused checks are not a cache/interoperability certification.

Both AppImages were built on Ubuntu 24.04 x86_64 from tracked inputs, with the fixed UI override identified by its hash. The fixed artifact ran the journey in a separate Ubuntu runtime with no checkout/Rust. Shell startup, owned tone generation and the external mpv null-output probe passed there. Packaging/runtime notice and physical playback certification remain open. No artifact upload or release occurred.

For an isolated Ubuntu test environment, install the optional harness prerequisites:

```sh
sudo apt-get install --no-install-recommends orca speech-dispatcher-espeak-ng espeak-ng pulseaudio xdotool dbus-x11 at-spi2-core xvfb xauth python3-gi
```

Run only in a disposable D-Bus/Xvfb session against the synthetic P01 shell. Evidence output refuses overwrite:

```sh
dbus-run-session -- xvfb-run -a /usr/bin/python3 tests/desktop/orca.py /absolute/path/Feishin-Rust-P01-x86_64.AppImage /tmp/p01-new-orca.json
```

The default Python suite does not require GUI/audio/Orca packages. Its regressions check strict utterance filtering, private service configuration, log bounds, forced process cleanup and saved evidence consistency. The native workspace now passes 33 default Rust tests; five real native probes and two performance experiments remain explicitly opt-in. Full keyboard/focus workflows, IME/RTL/fonts, DPI/touch, other screen readers/OS/browser targets, human speech usability and calibrated resource/input/presentation budgets remain required. Default/native-WASM-wgpu Clippy, Windows x64 Clippy, macOS arm64 compile, formatting, release web bundles, all 56 Python tooling tests with frozen reference inputs, inventory, seven local reference regressions, node syntax and documentation/whitespace checks passed. Both independent reviewers accepted the final change. Both found the same cleanup defect: a service leader could exit while a descendant ignored SIGTERM. The bounded group kill/subreaper fix and real descendant regression passed; both reviewers independently reran all seven Orca tooling tests and verified the refreshed evidence hashes. No actionable findings remain.
