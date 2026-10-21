# P00 artifact verification

Date: 2026-10-21. This record validates the completed evidence/tooling increment, not P00 exit. Runtime gaps are in the [task matrix](README.md).

| Check | Result |
| --- | --- |
| `python3 tools/app/check-p00.py --archive docs/upstream/feishin-7d6ad318.tar.gz --functions docs/inventory/functions.jsonl` | Passed: tracked links/fixtures/journey mappings, source and 443 asset/configuration hashes against pinned archive, stable journey IDs |
| `P00_TYPESCRIPT=/tmp/feishin-p00-tooling/node_modules/typescript/lib/typescript.js python3 -m unittest discover -s tests/tooling -p 'test_*.py'` | 10 passed, no skips: page bounds/units/identity, evidence/source-versus-target checks, CSV line endings, multiline JSX route/resource-query regression, function IDs, parser version and syntax failure paths |
| Regenerate both optional harness reports to `/tmp`, then `cmp` with tracked results | Passed, identical; source declarations and 6 positive/6 negative DTO fixtures rerun |
| `python3 tools/verify-inventory.py` | Passed: immutable archive/inventory and local reference tracking integrity |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 6 passed; these remain ignored analysis-tool tests |
| `node --check` for `tools/inventory-upstream.cjs`, `tools/app/p00-source-audit.cjs`, `tools/app/p00-wire-check.cjs` | Passed |
| `git diff --check`, final `git diff --cached --check` and explicit `git check-ignore --no-index` assertions | Passed after CSV LF normalization; references ignored, app docs/tools/fixtures trackable |
| Copy maintained inputs to clean temporary tree without inventory/archive/reference plans or dependency installations; run default checker and suite | Passed: checker; 7 default tests passed and 3 optional TypeScript parser tests explicitly skipped |
| `git check-attr text eol` plus temporary checkout with `core.autocrlf=true` | Passed: focused [.gitattributes](../../../.gitattributes) rules preserve LF for all three CSVs on Windows-style checkouts |
| Pinned web and remote production builds | Passed with warnings documented in [reference run](reference-run.md) |

Inspection found and corrected an initial multiline JSX attribute extraction error, CSS `?inline` resolution and function-ID binding locations before final checks. Regression tests cover each. The wire harness's initial esbuild alias error was corrected to use the TypeScript paths mapping, then the schema/normalizer harness was rerun successfully. No failing result is presented as a pass.

The final staged whitespace check additionally exposed CRLF line endings in the three new CSV ledgers, which unstaged `git diff --check` had not covered. They were normalized to LF, and the checker now rejects carriage returns in maintained CSVs with a regression test. Focused review identified a Windows checkout risk; `.gitattributes` now forces LF only for the maintained baseline/migration CSV paths. The affected checks and a `core.autocrlf=true` checkout simulation passed before commit.

No Cargo workspace exists, so Cargo formatting/Clippy/tests and native/WASM app builds are not applicable. No desktop/browser UI launch, screenshot/drag/key flow, live server, authenticated audio, OS/capture/cast device or numerical runtime performance check passed or was substituted by static evidence. The source-level harness uses explicit stubs and synthetic values; complete model/HTTP/error parity is not asserted.

Two independent reviewers inspected the actual changes: correctness/compatibility/architecture, and tests/edge cases/performance/operations. Both raised the same P2 finding: journey expectations mixed target improvements with uncaptured original outcomes. The fix renames `expected` to `target_expected`, labels rows `source-anchored-target-proposal`, records `reference_outcome=not-captured`, links a governing ADR per row, and updates migration status/checker/docs. A regression rejects the former ambiguous labeling. Both reviewers inspected that fix and subsequent LF/checker/Windows-checkout corrections, independently reran relevant tests and artifact/staged checks, and reported final satisfaction with no remaining actionable findings. The completed evidence/tooling increment is checked and reviewed; P00 remains open.
