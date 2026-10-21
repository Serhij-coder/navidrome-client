# Project scratchpad

- Stage: P00 baseline characterization open; no Cargo workspace or Rust application.
- Direction: egui desktop UI; native performance/usability first; appearance may differ from Feishin.
- Present: [P00 evidence](baseline/README.md), 52 source-anchored target journeys, route/import audit, synthetic fixtures and tooling.
- P00.01/.02/.05: baseline verified, static route/journey work and bounded P01 criteria recorded.
- Partial P00.03/.04/.06/.07/.08: see evidence matrix; no phase exit or runtime parity claim.
- Pipeline: implement -> tests -> fix/retest -> two independent reviews -> fix/retest -> commit/push.
- Reference: pinned web/remote production builds pass with lifecycle scripts disabled; desktop not launched.
- Reference evidence: local rewrite plan, inventory/archive and analysis tools remain ignored.
- Checks: 10 tooling + 6 inventory tests, hashes/links/ignore/syntax and staged checks pass; both reviews satisfied after fixes.
- Blockers: no controllable browser; live server captures, full behavior traces and calibrated hardware/audio measurements missing.
- Git: main tracks origin/main (SSH remote); verify task delivery with git status.
- Next: close P00 runtime/measurement/license gates; do not start P01 yet.
- Rules and detailed documentation: [AGENTS.md](../../AGENTS.md), [app documentation](README.md).
