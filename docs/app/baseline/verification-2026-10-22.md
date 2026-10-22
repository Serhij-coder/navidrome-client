# P00 non-UI increment verification

Date: 2026-10-22. Scope: P00.04/.06/.07/.08 evidence and reference tooling only. No P01 workspace, production Rust code, release, deployment or original source modification. The prior increment's [verification](verification.md) remains a historical record.

| Check | Result |
| --- | --- |
| Tooling suite with `P00_REFERENCE_ROOT`, `P00_ESBUILD`, `P00_TYPESCRIPT` | 22 passed, including exact store/callback evidence reproduction and redaction/bounds/license/cleanup tests |
| Same suite and default checker in a clean input copy without ignored source/dependencies | 17 passed, 5 optional reference tests explicitly skipped; checker passed |
| `check-p00.py --archive ... --functions ...` | Passed, including 18 new stable runtime/HTTP source anchors, original archive hashes and local links |
| Local `verify-inventory.py`, inventory unittest discovery, analysis-script syntax | Passed; 6 inventory tests, immutable baseline intact |
| Node syntax for loader and three reference scenario scripts; Python collector CLI/help/import | Passed |
| Final live collector run | 92 redacted responses, six old/current server instances, all configured admin/reader accounts authenticated; Navidrome/Jellyfin native roles checked; known generated password/token values absent from saved output |
| Duplicate playlist mutations | All four Subsonic-compatible instances: create own two-entry duplicate playlist, remove one index, observe one remaining occurrence, delete own playlist |
| Container cleanup | All six task-created servers stopped; no matching running containers after capture |
| Queue timing run | 40 raw samples after stopping fixture services; five samples per operation/size, no percentile or packaged-runtime claim |
| Font evidence | 12 archived font hashes cross-reference asset ledger; four publisher license texts retained byte-exact in JSON and verified by decoded UTF-8 SHA-256 |

The first live bootstrap attempts exposed genuine version/configuration differences: Navidrome uses `username` for first admin creation versus `userName` for native user creation; Jellyfin needs first-user initialization before wizard completion; old Gonic lacks the playlist-directory flag; Gonic salted auth uses its stored password string, so an initial bcrypt fixture seed failed. The fixture setup was corrected against official source/help and every affected authentication/scan/capture was rerun successfully. A transient Jellyfin post-restart authentication failure cleared after service initialization; the failed run wrote no output, and the later completed capture passed. These failures are not labeled passed tests or patched upstream behavior.

Whole-module headless storage tests additionally confirmed loss of legacy `autoDJ` and stale settings category writes; exact callback traces confirm media-time accumulation and oversized submission arguments. See [characterization](headless-characterization.md) and [ADR-009](../adr/009-source-differences.md). Neither source callback arguments nor direct service probes establish downstream adapter/audio/device behavior. A last-item stop-event identity ambiguity remains unresolved.

Publisher OFL texts contain original trailing whitespace. Their text is preserved in JSON string containers with decoded-text hashes, avoiding byte changes, checkout-dependent hashes or whitespace-rule exceptions. No font binary or application dependency was selected. Current publisher copyright years do not prove exact-version rights for every archived binary.

Two independent reviews inspected the actual changes. The correctness/compatibility/architecture reviewer found no actionable issues. The verification/edge-case/operations reviewer found that playlist cleanup ignored its response; the collector now requires HTTP success and a successful protocol envelope before completing. Regression tests cover HTTP failure, protocol failure/missing status and safe diagnostics, and validate all four saved successful live deletions. The verification reviewer approved the fix after independently rerunning all 22 tests and evidence checks; the correctness reviewer also approved the focused change after rerunning all 12 affected runtime-evidence tests. The saved live artifact required no change.

P00 remains open for interactive original captures, full hook/adapter/engine flows, live large-library cases, packaged desktop/browser performance and audio/hardware calibration, plus remaining release/font provenance decisions. Screenshots alone cannot close those gates.
