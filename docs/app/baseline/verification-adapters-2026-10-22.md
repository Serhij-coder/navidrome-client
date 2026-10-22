# P00 adapter increment verification

Date: 2026-10-22. Scope: P00.04/P00.06 source characterization and optional reference tooling. No original source, live profile, Cargo workspace or later-phase implementation changed. The prior [non-UI verification](verification-2026-10-22.md) remains a separate historical record.

Implementation adds an offline Axios boundary around the pinned original controllers/clients/interceptors/normalizers and retains [54 cases](../../../tests/fixtures/p00/adapter-traces.json). Saved live body/status records and synthetic inputs are labeled separately, with input hashes and explicit side-effect boundaries. It reuses existing captures without starting services or sending new HTTP requests.

| Check | Result |
| --- | --- |
| Tooling suite with all three optional reference variables | 31 tests passed, no skips; includes exact adapter report reproduction, origin rejection, modified-source rejection and autocrlf checkout regression |
| Source/archive/stable-ID/link checker | Passed with the original archive and inventory IDs; 14 additional source anchors |
| Clean-input suite without ignored sources/dependencies | 31 discovered: 24 passed, 7 optional reference checks explicitly skipped; default evidence checker passed |
| Local inventory integrity/tests and Node syntax | Passed; 6 inventory tests, both new JS files and original inventory script checked |
| Independent correctness/architecture review | Satisfied, including focused re-review of LF checkout protection and prerequisite/snapshot wording; no actionable findings |
| Independent verification/operations review | Found cross-platform raw fixture hash risk; fixed and independently rechecked with all 31 tests, archive/ID/link checks and LF attributes; no remaining actionable findings |
| Staged scope/whitespace/private-data inspection | Passed for the 18 intended task files; no ignored reference inputs or generated fixture credentials staged |

The first harness executions exposed missing VM globals and a cross-realm assertion mismatch. Explicit TextEncoder/browser identity globals and a plain-data comparison fixed those harness setup failures; affected replay and the full tooling suite passed afterward. No upstream code was patched to obtain a passing report.

Review identified that `core.autocrlf=true` could change the exact bytes bound by input hashes. Git attributes now force LF for fixture JSON. A regression checks out both hash-bound inputs from a temporary index and compares bytes, with an unpinned text control required to convert to CRLF. The initial test prototype retained matching working files, so inspection caught that it did not force conversion; deleting them before checkout fixed the regression itself. Both reviewers approved the final fix after affected checks passed.

Findings and limits are in the [adapter guide](adapter-characterization.md) and [ADR-009](../adr/009-source-differences.md). The report does not claim schema conformance for responses that emit diagnostic warnings. It does not establish live authenticated Feishin traffic, remote mutation behavior, full React/engine integration, failed detached Jellyfin requests, server/plugin use of submission ticks, live large libraries or UI/audio/performance parity. P00 remains open.
