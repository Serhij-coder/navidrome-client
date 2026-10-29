# P01.05/P01.07 hosted native probe diagnostics

The [96a8a6d hosted run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40467647446) failed only the Ubuntu 24.04 x86_64 AppImage job. Its native-control step failed with exit code 101; capture, packaging and artifact smoke steps were skipped. The other six jobs passed, including the aarch64 AppImage job. This failure happened before AppImage creation and does not identify an AppImage runtime failure.

The [saved record](evidence/native-probe-ci.json) contains the public job/annotation evidence and local reproduction results. GitHub's job-log API returned HTTP 403 and no browser session was available. The annotation contains no Rust test name or assertion. The combined step invokes direct control routing, repeated handoff and private-bus MPRIS probes in order; the public evidence cannot identify which one failed. No cause or production behavior fix is claimed.

Ten local repetitions of all three exact commands passed on Ubuntu 24.04 x86_64 with mpv 0.37.0 and null output: 30 invocations total. Each handoff invocation performs four replacement/readiness/seek cycles. The new wrapper mode also passed all three commands once. These passes do not reproduce or explain the hosted failure. The earlier unexplained MPRIS seek deadline remains a separate lead, not a diagnosis of this run.

The packaging jobs now invoke:

```sh
python3 tools/app/test-native.py --native-controls
```

This mode runs the same three commands, in the same order, and stops at the first nonzero exit. It streams the normal Cargo output and emits only validated failed Rust test identifiers as public CI annotations. Assertions, child output and arbitrary payloads are not copied into annotations. Detailed logs retain normal Cargo output. There is no retry, deadline change, ignored-failure conversion or removed test. Existing default workspace and `--external-audio` scopes are unchanged; modes are mutually exclusive.

The [tooling regressions](../../../tests/tooling/test_p01_native_ci.py) exercise exact scopes/order, failure at each of the three commands, exit propagation, annotation filtering and option rejection before process execution. The final tooling suite ran 77 tests: 69 passed and eight optional reference reproductions were skipped because their external parser/reference inputs were unavailable. Inventory, seven local analysis-tool tests, JavaScript syntax, documentation links and whitespace were checked separately. Rust implementation was unchanged, so this increment uses the real three-probe wrapper execution rather than claiming a fresh workspace/platform certification.

The next required evidence is the original failing assertion or a newly annotated hosted failure, followed by a targeted reproduction and regression fix. Both independent reviews must pass before the diagnostic change is committed. Review outcomes and exact checks are recorded in the saved record. P01, native fallback selection and platform/device gates remain open.

Hosted follow-up: [b10c2ac run 40468735553](evidence/hosted-native-probe-diagnostics.json) passed all seven jobs, including both AppImage CPUs and native Linux/Windows/macOS. Both independent reviewers found no actionable findings in the diagnostic change. This run validates the new wrapper under hosted CI, but does not identify or prove a fix for the earlier intermittent failure. A failing assertion is still needed if that failure recurs.

## Recurring hosted failure and source locations, 2026-10-29

The [4ac9367 run](https://github.com/Serhij-coder/navidrome-client/actions/runs/40960569900) passed native, portable/tooling, WASM and aarch64 AppImage jobs, but the [x86_64 native-control step](https://github.com/Serhij-coder/navidrome-client/actions/runs/40960569900/job/123751934957) failed before packaging. Its public Native Rust regression annotation identifies real_private_bus_mpv. The assertion details still require GitHub sign-in; no assertion or cause has been observed. The original failing excerpt was requested while independent work continued.

Sixteen direct local invocations of that exact ignored test passed in the existing owned Ubuntu 24.04 container with Rust/Cargo 1.92.0 and mpv 0.37.0. The complete --native-controls wrapper also passed its three exact tests. Tests use private D-Bus, owned synthetic tones and null audio. These passes do not reproduce, diagnose or fix the hosted failure; they do not certify audio hardware or production MPRIS.

The wrapper now additionally emits a fixed public source-location annotation for a Rust panic header ending in one of four allowlisted top-level tests: mpris.rs, control_session.rs, handoff.rs or prepared_handoff.rs. It reconstructs only the repository filename and bounded positive decimal line/column. It excludes the thread name, checkout prefix, assertion text and any following payload. This makes a future failure's source line visible through public check annotations. Unsupported files/formats produce no new annotation. Existing test-name annotations, normal log streaming, exact commands/order and nonzero exits remain unchanged; no retry, longer deadline, bypass or engine behavior change is introduced.

Seven focused tooling tests passed, including malicious suffix/path/newline rejection, private-marker exclusion, CI-only emission and unchanged failure propagation. One independent Sol medium reviewer found no actionable findings and independently passed all seven focused tests and scoped diff checks. Full current tooling passed 90 tests with nine optional skips (99 total), including the separate uncommitted enrichment tests; tracked links/evidence, inventory integrity, seven local reference tests and Node syntax checks passed. The P03 enrichment increment remains separate and uncommitted during this diagnostic task; its implementation does not establish a fix for this gate.
