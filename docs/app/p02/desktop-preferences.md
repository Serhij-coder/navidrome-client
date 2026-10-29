# Opt-in desktop preferences

The desktop now consumes the [storage worker](state-worker.md) when launched with an explicit isolated profile root. Without that flag, the existing P01 demo starts without opening application profile storage. This is P02.04/P02.05 shell consumption of the retained subset, not a working server-connected music client.

```sh
cargo run -p feishin-desktop --locked -- --profile-root /absolute/canonical/development-root
```

The path must be absolute with no parent traversal. Duplicate/missing values are rejected before GUI startup; CLI errors do not echo arguments. The worker performs filesystem checks/opening and writes only the existing `feishin-rust-dev-p02/` namespace below that root. Use a dedicated development directory, never the original Feishin profile. Existing renderer/smoke flags remain available; the profile flag applies to the GUI launch, not the separate playback-probe commands.

The Preferences window shows the saved profile count/current name without connecting to a server. It edits preferred language and follow-system appearance. Language is retained only; interface translation is explicitly unavailable. Confirmed `follow_system_theme=true` selects egui's System preference; false selects Dark, matching the pinned source's disabled-follow/default-dark defaults. Custom light/dark theme identities and full source theme behavior remain unported. The P01 theme controls are hidden only in this opt-in mode so they do not compete with the persistent preference owner.

## Save, recovery and close

Startup/Reload read the active snapshot or validated legacy-v1 fallback. Defaults and partial recovery are never autosaved. The window shows recovery/failure information; no Save action is exposed without a valid confirmed snapshot. Draft edits do not change the live theme until acknowledged. Corrupt/future documents remain protected and require external preservation/recovery before reloading.

Save sends only the small preference DTO plus the confirmed revision. `StateRepository::prepare_preferences` checks the revision and creates the patched candidate from one exact target baseline on the worker. Profiles/selection, columns, Auto DJ, queue occurrences/current/position and the backup contract are preserved. The UI does not clone or serialize the entire durable state. Its matching ActiveReview token triggers Apply; only the matching AppliedActive acknowledgment publishes the snapshot and theme. Wrong generation/request/kind completions cannot publish or trigger a write. Failed/uncertain mutations require explicit Reload before another save; there is no automatic mutation retry or claim that an I/O error means no commit.

Cooperative window close cancels the viewport close while accepted prepare/apply work finishes, then observes worker shutdown/join before exiting. A save failure cancels that close request so the error stays visible; another explicit close can exit after the user has seen it. The UI continues rendering/polling while waiting. OS-blocked filesystem work can keep cooperative close waiting; forced process termination and machine power loss cannot provide this acknowledgment guarantee. No replacement worker is started over an old active writer.

The pinned AT-SPI adapter can report disabled buttons as Enabled/Sensitive (`accesskit_atspi_common 0.14.2`, `node.rs` state mapping). This surfaced in the native journey. When saving is unavailable, this window uses the inert text "Save unavailable" instead of offering a misleading Save action. This bounded workaround does not certify the rest of P01 accessibility or change the dependency.

## Evidence and scope

Exact source `7d6ad318c79802e226996cfc39acba6b05bb22e8`: `settings.store.ts:1360,1374,1433` supplies disabled-follow/English/default-dark; `useThemeSettings` at `3071:33` exposes broader theme fields still deferred. Separate source player/queue writes remain at `utils.ts:119-143`. This egui controller and preference-only transaction are target decisions in [ADR-004](../adr/004-persistence.md), with [migration anchors](../migration/p02-desktop.csv) and [verification](desktop-preferences-verification.md).

Native controls/save/reopen/future protection were executed on an isolated Ubuntu/Xvfb/private-bus session; [the redacted result](evidence/desktop-preferences.json) records 16 passing checks and the binary hash. The optional [runtime journey](../../../tests/desktop/preferences_runtime.py) requires explicit owned-display consent, AT-SPI, Xvfb, Openbox, xdotool and FFmpeg for optional screenshots. It reuses tracked cleanup helpers and task-owned profiles. Controller/storage tests remain ordinary Cargo targets/private includes; no external display or ignored reference source is required by default builds/CI.

With a binary built against compatible runtime libraries and those optional tools installed, use fresh output names:

```sh
dbus-run-session -- xvfb-run -a env P02_OWNED_INPUT_DISPLAY=1 LIBGL_ALWAYS_SOFTWARE=1 /usr/bin/python3 tests/desktop/preferences_runtime.py /absolute/path/feishin-desktop /tmp/new-preference-result.json --screenshot /tmp/new-preference.png
```

Profile editing/connection/authentication, import review/activation UI, background browser storage consumption, full localization/theme configuration, queue/playback restoration and P02.06 credentials/P02.07 cache/cancellation remain open. The demo table stays synthetic and independent from the saved profile/queue. Linux/WASM verification scope and all phase-exit limits remain unchanged.
