# P03.02 protocol foundation verification

Date: 2026-10-29. Scope: isolated continuation from `c877070` on `p03-protocol-foundation`, after the reviewed P03.01 construction foundation. The maintainer authorized this bounded continuation while P02 remains open. Root integration, one final independent review and delivery remain the primary orchestrator's responsibility. No P00/P01/P02/P03 exit is asserted.

## Source and fixture evidence

Read the immutable archive for Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8`, with source hashes matching the tracked P00 audit:

| Source | SHA-256 |
| --- | --- |
| `src/renderer/api/subsonic/subsonic-api.ts` | `5c10deab6015915945d656578d0a6aa54cee7fdbcb846734698fc3d04dcbe54e` |
| `src/renderer/api/subsonic/subsonic-controller.ts` | `c5afb4748f8b6eaa721d8388e656172326074e7f002cd90b0505e4c1ec348af0` |
| `src/shared/api/subsonic/subsonic-types.ts` | `06ecfdd15fd0b160ff404aa3cb54a6a87c4c02c8c1baa9f9943f0f9b68ebedc2` |

Exact source anchors cover response interception at API lines 394-425, silent transform 468-477, request callback 489-612, conditional discovery/controller lines 1432-1510, and wire schemas at types lines 3-9 and 409-422. [ADR-013](../adr/013-subsonic-protocol.md) separates static observations from deliberate target validation/error policies. The [migration CSV](../migration/p03-protocol.csv) binds eight scope/disposition rows to stable inventory IDs. This continuation did not execute original modules, installers, package hooks or new original-expression replay. Existing P03.01 replay evidence is retained without claiming it covers these new protocol paths.

The [new protocol test target](../../../tests/subsonic/protocol.rs) uses synthetic inline JSON and dummy credentials, with local `Rc`/`Cell`/`RefCell` doubles and explicit future polling. No live server responses, real credentials, sockets, profile or production timers are involved. Eighteen adapter tests pass in total: seven existing request/auth foundation cases and eleven protocol cases. Protocol coverage includes:

- Two-step discovery, optional/malformed fields, missing extension lists, unknown/duplicate names and explicit version membership.
- Form POST repeat encoding and content type, empty/future version rejection, explicit GET, optional fallback, and same-canonical-base authorization.
- Numeric/string authentication errors, code 0 failure, standard/unknown protocol errors, malformed JSON/envelopes, and HTTP categories including rejected HTTP 201/204.
- Request/response size, extension/version item limits, stale request and generation, pre-dispatch and post-completion cancellation, pending deadline rejection, equal-deadline acceptance and post-parse deadline checks.
- Closed network errors and redacted request/discovery/response diagnostics, including dummy backend-message and authenticated-URL text.

These tests do not independently wake a pending future or establish streaming allocation bounds; the documented transport cooperation remains a production integration requirement.

## Local checks

Rust/Cargo 1.92.0 used the established environment with `PATH=/tmp/feishin-p01-cargo/bin:$PATH`, `CARGO_HOME=/tmp/feishin-p01-cargo`, `RUSTUP_HOME=/tmp/feishin-p01-rustup` and independent `CARGO_TARGET_DIR=/tmp/feishin-p03-protocol-target`. The implementer reran all affected checks after the orchestrator's integration fixes; these final focused checks passed:

- `cargo test -p feishin-subsonic --locked --offline`: 18 passed, zero failures or ignored tests.
- `cargo fmt --all -- --check`.
- `cargo clippy -p feishin-subsonic --all-targets --locked --offline -- -D warnings`.
- `cargo clippy -p feishin-subsonic --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings`.

No registry dependency/version/checksum was added or changed. Existing `serde_json` became a production adapter dependency and the existing portable `feishin-domain` path dependency was added. The Cargo lockfile changes only the local adapter dependency list; no HTTP, executor, timer, native or desktop dependency is added. No workspace, CI, domain, cache, storage, application shell or shared scratchpad file was edited in this isolated task.

The second orchestrator also passed `python3 tools/app/check-p00.py` (tracked evidence and maintained Markdown links), `git diff --check`, and a focused eight-row CSV/stable-ID/test-anchor/LF validation against the immutable inventory. Parsed Cargo lockfiles confirm every registry package entry is unchanged and only the local adapter dependency list differs. Primary combined workspace/review evidence is recorded at integration. Original local reference checks already passed: `python3 tools/verify-inventory.py`, seven tests from `python3 -m unittest discover -s tools -p 'test_*.py'`, and `node --check tools/inventory-upstream.cjs`. These run in the ignored original reference environment; tracked application builds/tests do not depend on those files.

## Integration fixes and remaining evidence

Before handoff, the second orchestrator identified missing transport form content type, unredacted discovery version Debug, cross-server discovery reuse, unnecessary JSON cloning, overly broad successful HTTP statuses and missing request/generation/item/error edge cases. The implementer fixed them, added regressions, added post-parse context checks and reran the affected checks until passing. This integration inspection is not the required final independent review.

The primary orchestrator must assemble the patch with concurrent work, run applicable combined workspace/tooling checks, obtain one independent Sol medium review, resolve findings and only then commit/push. No independent-review result, combined run, commit, push or hosted CI result is invented by this isolated record.

Unimplemented evidence includes concrete native/browser transports, real server login/discovery, generic/extension-enabled conformance, TLS/redirect/CORS policy, secure entropy, credential-consent composition, actual socket cancellation/streaming bounds, jukebox and application feature-map composition. Form POST is planned but not sent to a service. No browser runtime, audio/device playback, packaged adapter or complete P03.02/P03 result is claimed. Current checks cover Linux/native and WASM compilation; Windows/macOS runs remain excluded.

## Primary assembly

The primary orchestrator assembled the nine isolated files onto `5c4096f`, alongside the disjoint storage serialization continuation and a concrete native handoff deadline-classification correction. Original destination baselines matched before assembly; no concurrent domain/cache edits were overwritten. Final pinned-toolchain combined checks passed: 234 workspace tests including four compile-fail doctests, 13 optional runtime cases ignored; workspace fmt/all-target Clippy; domain/storage/Subsonic/web/remote all-target WASM Clippy; P00/link checks; tooling tests (76 passed/nine optional skips); eight protocol and four serialization migration IDs; unchanged registry lock entries. The one final independent Sol medium reviewer inspected the complete combined scope and found no actionable findings. It independently passed 108 storage tests, all 18 Subsonic tests, the classifier regression, P00/documentation and diff checks. Fresh hosted CI is pending. The prior merge CI failure and its classification evidence are recorded in [the prepared-handoff correction](../p01/prepared-handoff.md#ci-deadline-classification-correction), without pretending that the original unprinted failure category was reproduced.
