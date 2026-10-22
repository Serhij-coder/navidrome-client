# Project scratchpad

- Stage: P00 baseline characterization open; no Cargo workspace or Rust application.
- Direction: egui desktop UI; native performance/usability first; appearance may differ from Feishin.
- Present: [P00 evidence](baseline/README.md), source journeys, synthetic data and 92 live responses from six disposable servers.
- P00.01/.02/.05: baseline verified, static route/journey work and bounded P01 criteria recorded.
- P00.04: old/current Jellyfin, Navidrome and Gonic with readers/extensions/playlist probes; all fixture services stopped.
- P00.06: 25 queue/persistence cases, 1,000 seeded commands and 8 progress traces; explicit headless boundaries.
- P00.07/.08: raw 10k/100k queue measurements and font notice snapshots added; desktop calibration/release rights still open.
- Pipeline: implement -> tests -> fix/retest -> two independent reviews -> fix/retest -> commit/push.
- Reference: pinned web/remote production builds pass with lifecycle scripts disabled; desktop not launched.
- Reference evidence: local rewrite plan, inventory/archive and analysis tools remain ignored.
- Checks: 22 tooling + 6 inventory tests, source/ID/link/license/syntax and clean-input checks pass; both reviews satisfied.
- Remaining: UI/screenshots, full hook/adapter/engine flows, live large libraries, hardware/audio calibration and exact font rights.
- Git: main tracks origin/main (SSH remote); verify task delivery with git status.
- Next: close P00 runtime/measurement/license gates; do not start P01 yet.
- Rules and detailed documentation: [AGENTS.md](../../AGENTS.md), [app documentation](README.md).
