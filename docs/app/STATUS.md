# Project scratchpad

- Stage: P00 baseline characterization open; no Cargo workspace or Rust application.
- Direction: egui desktop UI; native performance/usability first; appearance may differ from Feishin.
- Present: [P00 evidence](baseline/README.md), source journeys, synthetic data and 92 live responses from six disposable servers.
- P00.01/.02/.05: baseline verified, static route/journey work and bounded P01 criteria recorded.
- P00.04/.06: 54 offline adapter cases now follow saved responses/callback arguments through original clients; [limits](baseline/adapter-characterization.md).
- P00.06: 25 queue/persistence cases, 1,000 seeded commands and 8 progress traces; explicit headless boundaries.
- P00.07/.08: raw 10k/100k queue measurements and font notice snapshots added; desktop calibration/release rights still open.
- Pipeline: implement -> tests -> fix/retest -> two independent reviews -> fix/retest -> commit/push.
- Reference: pinned web/remote production builds pass with lifecycle scripts disabled; desktop not launched.
- Reference evidence: local rewrite plan, inventory/archive and analysis tools remain ignored.
- Checks: 31 tooling + 6 inventory tests and source/ID/link/syntax/clean-input checks pass; both reviews satisfied after LF checkout fix.
- Remaining: UI/screenshots, full hook/adapter/engine flows, live large libraries, hardware/audio calibration and exact font rights.
- Git: main tracks origin/main (SSH remote); verify task delivery with git status.
- Next: close P00 runtime/measurement/license gates; do not start P01 yet.
- Rules and detailed documentation: [AGENTS.md](../../AGENTS.md), [app documentation](README.md).
