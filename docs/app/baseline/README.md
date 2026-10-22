# P00 baseline and characterization

P00 is open. This evidence package completes the static route/journey work and records bounded P01 criteria. The maintainer subsequently authorized starting [P01 workspace/prototypes](../p01/README.md) on 2026-10-22 while these P00 gates remain open; this evidence package does not establish either phase exit.

The upstream baseline is jeffvli/feishin development commit `7d6ad318c79802e226996cfc39acba6b05bb22e8`, package 1.17.0. The immutable local archive has SHA-256 `a3986aa97191bf31695473ffbfd7982eb9c106ed4390b59cd52b1070213d6bcb`. Inventory verification covers 1,415 files, 1,014 parsed files, 10,243 functions and 1,039 callable declarations. The archive/inventory remain ignored and are optional inputs to reference checks, never application build dependencies.

| Task | State | Evidence and remaining acceptance |
| --- | --- | --- |
| P00.01 | Verified existing baseline | Archive/inventory checks; [source audit](source-audit.json) records inspected source hashes. No immutable baseline was changed. |
| P00.02 | Static characterization complete | [Routes and composition](routes.md), all 52 [journeys](journeys.csv), stable [migration anchors](../migration/p00-map.csv). Target expectations/UI mappings are ADR-linked proposals, separate from uncaptured original outcomes; execution belongs to P00.03. |
| P00.03 | Expanded, partial | [Original browser/desktop/remote run](browser-characterization.md): 51 screenshots, keyboard selection, mouse drag, persisted queues/settings and redacted live requests/events. Full journeys and OS/device behavior remain open. |
| P00.04 | Expanded, partial | [Six old/current live server instances](live-servers.md), non-admin accounts and 92 redacted API responses; [54 pinned adapter replay cases](adapter-characterization.md) now cover selected controllers/clients/interceptors/normalizers. Selected authenticated Feishin UI reads now run against all six servers; live large-library and full mutation/permission flows remain open. |
| P00.05 | Recorded, feasibility pending | [ADR-001](../adr/001-egui-desktop.md), [ADR-002](../adr/002-target-matrix.md), [ADR-003](../adr/003-playback-boundaries.md). Platforms/engines remain candidates until measured. |
| P00.06 | Expanded, partial | [Headless source execution](headless-characterization.md): 25 queue/persistence cases, 1,000 seeded commands and 8 progress-callback traces; [adapter replay](adapter-characterization.md) follows submission arguments into request configurations. Selected real browser playback/pause and current-version export/import/reload now pass; full effects, Auto DJ races and historical exports remain open. C01 is an unused-helper finding; see the browser record. |
| P00.07 | Expanded, partial | [Actual 10k/100k headless queue samples](queue-measurements.json) complement [candidate budgets/protocol](performance.md). Packaged desktop startup/frame/audio/idle measurements and calibration remain unavailable. |
| P00.08 | Expanded, partial | [Notices](../../../THIRD_PARTY_NOTICES.md), [font metadata/full publisher licenses](font-license-evidence.json), asset dispositions and ADR-010. Exact-version copyright reconciliation, Symbola terms, final branding and future distributed dependency/native-binary notices remain release gates. |

P00 exits only with the original running in an isolated profile, representative server captures, full behavior traces, actual measurements and calibrated budgets. Static checks, successful bundling and synthetic data do not satisfy those gates. P00 calibration remains required for P01 performance acceptance. The authorized initial P01 prototypes do not waive those gates; P02-P14 remain unopened.

Run tracked checks from a clean checkout with Python 3.11+, Node 22.22.2+ and Git:

```sh
python3 tools/app/check-p00.py
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
node --check tools/app/p00-source-audit.cjs
node --check tools/app/p00-wire-check.cjs
```

The parser regression test additionally needs TypeScript 6.0.3 at `P00_TYPESCRIPT`; without it that optional reference test is explicitly skipped. Set `P00_REFERENCE_ROOT` and `P00_ESBUILD` as well to rerun the optional saved state/scrobble/adapter traces. Reference reproduction and the optional source/archive comparison are in the [run record](reference-run.md); check outcomes are in the [initial verification](verification.md), [2026-10-22 increment](verification-2026-10-22.md) and [adapter verification](verification-adapters-2026-10-22.md) and [UI verification](verification-browser-2026-10-22.md). Default tracked checks read only tracked inputs. No check claims audio/device/end-to-end parity.
