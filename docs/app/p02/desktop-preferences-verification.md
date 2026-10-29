# Desktop preferences verification

Date: 2026-10-29. Scope: P02.04/P02.05 opt-in native preference consumption. See [the contract](desktop-preferences.md). Current checks target Linux/WASM only. No platform, playback, localization or phase exit is claimed.

## Automated checks

Pinned Rust/Cargo 1.92.0 via the [established launcher](README.md#reproduce-checks), locked inputs and existing dependency versions. Cargo.lock adds only the desktop's existing-workspace storage and test-tempfile dependencies.

- Workspace tests passed 184, including four compile-fail doctests; 13 existing optional runtime tests ignored. After the close-error fix, desktop tests passed five controller and two CLI cases. The strengthened storage preference-patch regression also passed after adding columns/Auto DJ to the preserved fixture.
- Storage focused targets passed: state10 and session7. The preference patch retains adopted profiles/selection, transformed columns, Auto DJ and repeated queue/current/position data; stale revisions and invalid preferences fail without mutation.
- Workspace fmt/Clippy, desktop wgpu-only all-target Clippy and domain/storage/web/remote WASM all-target Clippy passed after the final UI/close changes.
- Two optional-runtime isolation regressions passed without GUI dependencies: reject non-owned displays before loading GUI/opening profiles, and preserve existing evidence files.

Tracked P00/link checks, immutable inventory verification, seven local reference tests, Node reference-tool syntax and diff checks passed. Tooling ran 82 tests: 74 passed/eight optional external-reference skips. Migration CSV/stable inventory IDs and the 16-check runtime evidence were validated. One independent Sol medium reviewer reported no actionable findings and approved the bounded increment. The reviewer independently passed desktop5 controller/2 CLI, storage state10/session7, both runtime-isolation tests and diff checking. Native GUI and broader checks were assessed from recorded evidence. The prior `1df585c` [hosted run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40843105916) passed all five Linux/WASM/AppImage jobs.

## Native execution

The host-built binary initially could not run in the Ubuntu 24.04 lab: its host glibc dependency was 2.43. The same pinned Rust toolchain and tracked Cargo inputs were then built offline inside Ubuntu 24.04 using its compiler/libraries. Development build artifacts and the GUI runtime lived in task-owned containers; runtime networking was disabled. No real profile or server was accessed.

The [runtime journey](../../../tests/desktop/preferences_runtime.py) ran under a private D-Bus/Xvfb 1280x900/Openbox session with Mesa software rendering. Actual AT-SPI text/check/action state and injected X11 keys verified legacy read-only startup, profile summary, language/appearance state, native save, double-save idempotence, preserved profile/legacy bytes, Reload, reopen, accepted-save close, monotone revisions, future-schema preservation and unavailable Save action, and default no-profile mode. All 16 checks passed in the final [result](evidence/desktop-preferences.json). The saved screenshot was visually inspected; controls/text fit the window. This is not physical input, screen-reader speech, Wayland, live OS theme-change, GPU/performance or production playback evidence.

The first journey failed only its disabled Save accessibility assertion: the pinned AT-SPI mapping exposes disabled buttons as Enabled/Sensitive. The application now exposes inert "Save unavailable" text when no save is possible, and the updated native assertion passed on the rebuilt binary. Root inspection also found that save failure could complete a pending close before the user saw the error. The controller now keeps the window open on failure, with a regression proving a second explicit close is needed. A successful pending save still completes before cooperative shutdown. A truly blocked OS call remains non-interruptible.

The task-owned runtime/compiler containers and temporary runtime image were removed after execution. Screenshot/build intermediates are ignored artifacts, not required build/CI inputs or release outputs.
