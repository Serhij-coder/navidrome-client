# P00 baseline and characterization

P00 is open. This evidence package completes the static route/journey work and records bounded P01 criteria. It does not authorize starting P01. No Rust workspace or application has been created.

The upstream baseline is jeffvli/feishin development commit `7d6ad318c79802e226996cfc39acba6b05bb22e8`, package 1.17.0. The immutable local archive has SHA-256 `a3986aa97191bf31695473ffbfd7982eb9c106ed4390b59cd52b1070213d6bcb`. Inventory verification covers 1,415 files, 1,014 parsed files, 10,243 functions and 1,039 callable declarations. The archive/inventory remain ignored and are optional inputs to reference checks, never application build dependencies.

| Task | State | Evidence and remaining acceptance |
| --- | --- | --- |
| P00.01 | Verified existing baseline | Archive/inventory checks; [source audit](source-audit.json) records inspected source hashes. No immutable baseline was changed. |
| P00.02 | Static characterization complete | [Routes and composition](routes.md), all 52 [journeys](journeys.csv), stable [migration anchors](../migration/p00-map.csv). Target expectations/UI mappings are ADR-linked proposals, separate from uncaptured original outcomes; execution belongs to P00.03. |
| P00.03 | Partial, blocked | Pinned production web and remote builds pass; [run record](reference-run.md). Browser tool has no available browser; no UI screenshots, keyboard/drag captures, live serialized profiles or request/event traces. Desktop launch also unverified. |
| P00.04 | Partial | [Synthetic fixtures](../../../tests/fixtures/p00/README.md), 12 server scenarios and bounded 10k/100k generator; [schema/normalizer results](wire-results.json). Live old/current/non-admin server runs and redacted captures remain missing. |
| P00.05 | Recorded, feasibility pending | [ADR-001](../adr/001-egui-desktop.md), [ADR-002](../adr/002-target-matrix.md), [ADR-003](../adr/003-playback-boundaries.md). Platforms/engines remain candidates until measured. |
| P00.06 | Partial | [Characterization and open cases](characterization.md), [ADR-009](../adr/009-source-differences.md). Selected exact declarations run in isolation; hook timing, speed, queue traces and actual export/import still need the reference UI. |
| P00.07 | Partial, blocked | [Numerical candidate budgets and protocol](performance.md). Hardware partly identified; original runtime measurements, GPU/display details and budget calibration unavailable. |
| P00.08 | Partial | [Notices](../../../THIRD_PARTY_NOTICES.md), [asset dispositions](assets.csv), [ADR-010](../adr/010-development-policy.md). Final release branding, dependency/native binary notices and font license verification remain release gates. |

P00 exits only with the original running in an isolated profile, representative server captures, full behavior traces, actual measurements and calibrated budgets. Static checks, successful bundling and synthetic data do not satisfy those gates. Keep P01-P14 unopened until those gaps are resolved.

Run tracked checks from a clean checkout with Python 3.11+ and Node 22.22.2+:

```sh
python3 tools/app/check-p00.py
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
node --check tools/app/p00-source-audit.cjs
node --check tools/app/p00-wire-check.cjs
```

The parser regression test additionally needs TypeScript 6.0.3 at `P00_TYPESCRIPT`; without it that optional reference test is explicitly skipped. Reference reproduction and the optional source/archive comparison are in the [run record](reference-run.md); exact check outcomes are in [verification](verification.md). The tracked checks read only tracked inputs. No check claims audio/device/end-to-end parity.
