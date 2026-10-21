# Application documentation

This directory contains maintained documentation for the Rust application. The root [README](../../README.md) is the public introduction; [AGENTS.md](../../AGENTS.md) contains the development rules. The project uses the GNU GPL version 3 license in [LICENSE](../../LICENSE).

Start with the brief [project scratchpad](STATUS.md) for current state, checks, blockers and next action. Keep detailed setup, behavior and design documentation here; the scratchpad is limited to 25 non-empty lines and is not a running history.

The desktop UI direction is egui, with native desktop performance and usability as priorities. Layout and appearance may differ from Feishin while retaining agreed features, effective workflows, accessibility and localization. Native audio, visualization, capture and browser/remote boundaries need implementation evidence; toolkit selection is not a measured performance result.

Rust implementation has not started. There is no Cargo workspace, runnable app or validated build command yet. Add setup, build/run/test, configuration, packaging and troubleshooting instructions here as working implementation becomes available; distinguish supported behavior from planned work.

Keep application test code and fixtures in the dedicated top-level [tests directory](../../tests/README.md), including tests of app tooling. P01 must wire those files into Cargo/test harnesses and CI; their location alone does not make them run.

Maintain application ADRs under `docs/app/adr/`, source-to-target migration progress under `docs/app/migration/`, and performance budgets/results here as those records are created. Record hardware and release-build conditions for startup, memory, idle CPU/GPU, frame/input latency and playback measurements. The current provisional large-list target is p95 frame time <=16.7 ms at 60 Hz for 100k-row scrolling; it is unverified and needs hardware-specific calibration.

Update app documentation, licenses/notices and relevant operational instructions in the same task as behavior or configuration changes. Keep required build/test instructions reproducible from tracked files. Local rewrite plans, inventory, reference archives and analysis tools outside `docs/app/` and `tools/app/` are ignored development references; application builds and CI must not depend on their presence.
