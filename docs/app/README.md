# Application documentation

This directory contains maintained documentation for the Rust application. The root [README](../../README.md) is the public introduction; [AGENTS.md](../../AGENTS.md) contains the development rules. The project uses the GNU GPL version 3 license in [LICENSE](../../LICENSE).

Start with the brief [project scratchpad](STATUS.md) for current state, checks, blockers and next action. Keep detailed setup, behavior and design documentation here; the scratchpad is limited to 25 non-empty lines and is not a running history.

The desktop UI direction is egui, with native desktop performance and usability as priorities. Layout and appearance may differ from Feishin while retaining agreed features, effective workflows, accessibility and localization. Native audio, visualization, capture and browser/remote boundaries need implementation evidence; toolkit selection is not a measured performance result.

Rust implementation has not started. There is no Cargo workspace, runnable app or validated build command yet. Add setup, build/run/test, configuration, packaging and troubleshooting instructions here as working implementation becomes available; distinguish supported behavior from planned work.

P00 now has tracked [baseline evidence and open gates](baseline/README.md): route/composition analysis, 52 source-anchored target journeys, synthetic fixtures and [controlled reference builds](baseline/reference-run.md). The 2026-10-22 increment adds [six live old/current server fixtures](baseline/live-servers.md), [headless queue/persistence/scrobble traces](baseline/headless-characterization.md), raw 10k/100k queue timings and font notice evidence. Interactive reference runs, full adapter/engine integration, live large libraries, audio/device behavior and calibrated desktop measurements remain incomplete. P01 has not started. Run the tracked evidence/tooling checks documented in the baseline guide; optional reference checks are separate.

The subsequent [adapter characterization](baseline/adapter-characterization.md) adds 54 original controller/client/interceptor/normalizer replay cases, following saved server responses and callback arguments through an offline Axios boundary. It records partial adapter behavior and payloads separately from live authenticated application traffic.

Keep application test code and fixtures in the dedicated top-level [tests directory](../../tests/README.md), including tests of app tooling. P01 must wire those files into Cargo/test harnesses and CI; their location alone does not make them run.

Maintained ADRs record [egui direction and P01 criteria](adr/001-egui-desktop.md), [target matrix](adr/002-target-matrix.md), [playback boundaries](adr/003-playback-boundaries.md), [source differences](adr/009-source-differences.md) and [development identity/privacy/update policy](adr/010-development-policy.md). Source-to-target [migration anchors](migration/p00-map.csv) remain unmapped; no Rust port is claimed. [Performance budgets/protocol](baseline/performance.md) distinguish uncalibrated numerical candidates from unavailable runtime results, including the provisional p95 <=16.7 ms at 60 Hz for 100k-row scrolling.

Update app documentation, licenses/notices and relevant operational instructions in the same task as behavior or configuration changes. Keep required build/test instructions reproducible from tracked files. Local rewrite plans, inventory, reference archives and analysis tools outside `docs/app/` and `tools/app/` are ignored development references; application builds and CI must not depend on their presence.
