# P01.06 keyboard track actions and clipboard

The synthetic table now opens a focused track action with Shift+F10 while its table navigation control has keyboard focus. The menu focuses its copy button; Enter copies the focused title and Escape cancels. Both return focus to the table without changing the selected range. A visible shortcut hint and an accessibility description make the action discoverable. The existing row pointer menus remain available.

The action records the focused synthetic record and view generation when opened. Pending, missing or filtered-out focus cannot open it. A changed generation or focused record cancels an existing menu; closing from an outside text input preserves that input's focus. Membership is checked only on the opening user command, not by scanning the catalog every frame. The menu retains one target and one counter, clones one title, and adds no dependency, profile storage or production queue behavior.

A dismissed menu item gets a fresh accessibility identity on reopening while its popup Area identity stays stable. The initial candidate could focus the reopened item through AT-SPI but Orca did not announce it again. The final identity strategy and input-driven follow-up repaints pass the repeated native/screen-reader journeys. The exact backend/reader cause was not isolated; this is a bounded local treatment of the observed silence, not a general upstream defect claim. A 64-opening regression checks fresh item IDs, focus return and bounded egui widget-data entries; this is not a complete heap/resource or platform certification.

The [five Rust regressions](../../../tests/spike-ui/track_actions.rs) are included under the owning crate's test boundary. They cover keyboard opening/copy/cancellation, range and offscreen-boundary targets, absent/pending/hidden focus, generation invalidation, outside-input focus and repeated menu lifecycle. The clipboard output command checks are separate from the optional real native clipboard journey.

The [native journey](../../../tests/desktop/track_actions.py) uses actual AT-SPI focus/text state, X11 keys and the private X11 clipboard. It checks first/range/last-track copies, immediate reopening, Escape/focus return, a real copy-to-filter paste and the completed filtered view. It requires explicit owned-display consent and local X11/Unix D-Bus endpoints before importing GUI bindings. The caller must create the disposable D-Bus/Xvfb session; the flag does not itself prove isolation. Only the known synthetic title is compared, and raw clipboard/accessibility/process data are excluded from saved evidence. Owned profiles and bounded process cleanup reuse the tracked Orca harness.

The [Orca journey](../../../tests/desktop/orca.py) keeps its original nine checks by default. Its optional `--track-actions` mode adds ten open/cancel/reopen/copy and repeated-cycle speech-request checks, using the real Orca CLI with a private Speech Dispatcher/eSpeak NG null-sink setup. Speech evidence still admits only exact known synthetic phrases. Neither journey certifies physical keyboard latency, human speech quality, Wayland, Windows/macOS, browser/mobile clipboard, touch, IME or RTL shaping.

Optional Ubuntu 24.04 prerequisites:

```sh
sudo apt-get install --no-install-recommends python3-pyatspi python3-gi xclip xdotool dbus-x11 at-spi2-core xvfb xauth orca speech-dispatcher-espeak-ng espeak-ng pulseaudio libxkbcommon-x11-0 libegl1 libgl1 libgl1-mesa-dri
```

Run against the synthetic feasibility binary only, on newly provisioned sessions. Evidence paths must be new:

```sh
dbus-run-session -- xvfb-run -a env P01_OWNED_INPUT_DISPLAY=1 /usr/bin/python3 tests/desktop/track_actions.py /absolute/path/feishin-desktop /tmp/p01-new-track-actions.json
dbus-run-session -- xvfb-run -a env P01_OWNED_INPUT_DISPLAY=1 /usr/bin/python3 tests/desktop/orca.py /absolute/path/feishin-desktop /tmp/p01-new-track-actions-orca.json --track-actions
```

Upstream anchors are `src/renderer/components/item-list/item-table-list/item-table-list.tsx:193:30` and `src/shared/components/context-menu/context-menu.tsx:73:1` (Radix root/provider), `:116:1` (item disabled/selection handling) and `:147:1` (trigger). This task replaces presentation for one synthetic action, not the original production action catalog. P07-P09 mutations/navigation and OS/browser input matrices remain deferred. No upstream defect or full context-menu parity is claimed.

The [manifest](evidence/track-actions-manifest.json) binds historical source inputs, release binary digests, the [baseline failure](evidence/track-actions-before.json), [15 passing native clipboard checks](evidence/track-actions-fixed.json), the [intermediate repeated-menu speech failure](evidence/track-actions-orca-before.json) and [19 passing Orca checks](evidence/track-actions-orca-fixed.json). The historical Orca source digest predates a follow-up guard requiring both local X11 and Unix D-Bus endpoints in addition to consent; current validation is recorded separately. Both native journeys were rerun with that guard and produced byte-identical evidence. The builds are native release binaries in an isolated Ubuntu builder, not new AppImage/off-checkout or physical-device certification. P01 exit and P02 are not claimed.

Final integration used a separate source snapshot containing tracked inputs and the pending task files, Rust 1.92.0 in the Ubuntu builder, and the frozen optional reference inputs for tooling. These commands passed:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p feishin-web -p feishin-remote --target wasm32-unknown-unknown --locked -- -D warnings
cargo clippy -p feishin-spike-ui --all-targets --target x86_64-pc-windows-msvc --locked -- -D warnings
cargo clippy -p feishin-desktop --no-default-features --features wgpu --all-targets --locked -- -D warnings
python3 tools/app/check-p00.py
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
python3 tools/verify-inventory.py
python3 -m unittest discover -s tools -p 'test_*.py'
node --check tools/inventory-upstream.cjs
git diff --check
```

All 54 default Rust tests, 68 tooling tests with reference inputs and seven analysis-tool tests passed. Ten external native/performance tests remained explicitly ignored. Manual checks verified all 17 P01 migration anchors/evidence paths, reference ignores/application exceptions and the historical source/binary/evidence digests, except the documented current Orca guard delta. Initial stale workspace artifacts required cleaning the seven workspace package caches before the passing Rust rerun. The raw-binary native rerun initially lacked `libxkbcommon-x11`; after installing that runtime library and `xclip` in the disposable environment, all 15 clipboard and 19 Orca checks passed. No application source fix was needed for either environment issue. Both independent reviews inspected the actual code, tests and evidence and reported no actionable findings. Each independently passed the 11 focused tooling tests and whitespace check; the operations review also verified evidence/current source digests. Their host lacked Cargo, so Rust/native results rely on the orchestrator builder and runtime runs above.
