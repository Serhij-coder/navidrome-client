# P02.06 credential executor handoff

Date: 2026-10-29. Isolated continuation from `1a99711` on `p02-credential-executor`. This adds scheduling around the existing [credential owner](credentials.md) and optional Linux adapter under [ADR-006](../adr/006-credentials.md). It does not complete P02.06 or a phase exit. Authentication UI, backend login and P02.07 are outside this change. `STATUS.md` is intentionally left for the primary orchestrator's combined integration record.

## Contract and composition

[credential_executor](../../../crates/storage/src/credential_executor.rs) provides typed `Command`, `Request`, `Response` and `Reply` values. `Generation` and `RequestId` are distinct nonzero IDs. Every reply retains the submitted generation and request ID, including rejected requests. The caller must allocate a fresh generation when replacing an owner and check both IDs before consuming a reply. The portable `Dispatcher` accepts a non-Send provider and caller-sampled monotonic `Duration`; it performs synchronous work and is not a browser background worker.

`Put`, `Get` and `Clear` operate on session memory only. `Remember`, `Restore` and `Forget` are separate explicit actions. Default dispatcher/worker construction uses `NoPersistence`, without provider lookup or automatic restore. Remember/restore remain Unsupported with that provider, and forget clears the local value. Remember failure keeps the session usable; forget clears session memory before durable deletion and can therefore fail with a durable copy remaining. Existing namespace/account/endpoint/slot separation and owner limits (128 entries, 64 KiB per value) are preserved.

[credential_worker](../../../crates/storage/src/credential_worker.rs) is native-only. `CredentialWorker::new(generation, capacity)` starts a session-only worker; `with_factory` constructs a provider on the worker thread. The factory must be Send, while its returned provider need not be. With Linux and `linux-secret-service` enabled, compose the existing lazy adapter explicitly:

```rust,ignore
CredentialWorker::with_factory(generation, 2, || Ok(LinuxCredentials::new()))?;
```

The factory and all provider operations run off the caller thread. This is library composition, with no shell, authentication, domain `SecretStore` or persisted-consent integration. Production Linux composition must be chosen explicitly; enabling the feature alone does not connect to a keyring.

## Bounds, deadlines and outcomes

Native capacity must be 1-8. A slot covers a queued request, the in-flight request or its completed unread reply, and is released only when `poll` consumes that reply. Both channels are bounded to capacity; at most capacity replies can be produced before consumption, so unread replies cannot prevent cooperative shutdown through a full reply channel. Admission rejects oversized `Put` values before enqueueing and reports invalid capacity, full/closed admission and ID/time exhaustion separately. Retained credentials and outstanding secret replies remain bounded independently. These limits do not bound every allocation made by a provider/SDK or caller before admission.

`queue_wait` is a latest-start allowance measured from submission in the worker's monotonic epoch, not a provider execution timeout. Zero allowance expires before execution; reaching the deadline exactly also expires. Dispatch rejects wrong generations, cancellation and expiry before invoking the owner. `cancel` returning true means a cancellation request was recorded for an outstanding ID, including an unread reply; it does not acknowledge cancellation of an in-flight operation. Work that already passed the start gate keeps its actual result, even if cancellation, deadline expiry or shutdown follows.

`DispatchError::Operation` preserves closed credential error categories. Errors from `Remember` and `Forget` are conservatively marked `MutationUncertain`, including errors that a particular provider may know occurred before mutation. The existing persistence trait does not carry that stronger outcome evidence. The nested error still distinguishes Unsupported, MissingSession, Locked, Unavailable and other categories. No failed mutation is advertised as safely retryable. Reconcile the provider state and desired retention policy before retrying an uncertain operation. Restore does not perform a durable mutation, but can change session memory on successful completion.

`begin_shutdown` closes admission and requests cancellation of queued work without waiting. `poll` and `try_join` are nonblocking; `try_join` joins only a finished thread. Factory errors and worker panic are explicit join outcomes. Shutdown completion alone does not acknowledge each mutation: consume and correlate its reply. A missing reply after panic/disconnection or dropping a worker leaves any started mutation uncertain. Keep the outstanding request ledger at the application boundary and reconcile before resuming mutations.

Drop requests shutdown and detaches. An in-flight operation can retain secrets and continue after the handle is dropped. Before opening another owner over the same credential namespace, explicitly shut down and observe the previous worker's completion; a fresh generation does not fence an old provider write. There is no concurrent-writer or crash-recovery coordinator here.

## Provider prompt and privacy limits

The Linux adapter still uses encrypted DH sessions, fixed selectors and closed errors. Its underlying Secret Service create/delete APIs can execute provider prompts even after unlocked prechecks. D-Bus method timeouts do not establish a hard end-to-end prompt deadline. This executor does not dismiss prompts, interrupt blocking provider calls or kill threads. A prompt can keep `try_join` pending indefinitely. UI composition must continue repainting while shutdown is pending and must not claim remembered credentials were revoked without a confirmed outcome. A hard shutdown/prompt bound requires further provider-aware integration or an isolated helper protocol with explicit uncertain-outcome handling.

Secret-bearing commands, requests, responses and replies have no Debug or serialization implementation. Only fixed error categories and non-secret correlation IDs are suitable for diagnostics. Secrets stay in typed in-process payloads; there is no command-line, environment, profile/journal or diagnostic transport. Providers must likewise avoid secrets in their own diagnostics/panic messages. Session `Get` creates a bounded owned copy for the reply; this adds no secure-memory-erasure guarantee to the existing `Secret` type or SDK copies. No credential is read from a live Feishin profile.

## Traceability and integration needs

This executor replaces scheduling at the target application boundary; the pinned source has no equivalent Rust worker. The existing [P02.06 migration map](../migration/p02-credentials.csv) remains the policy/provider anchor. Relevant stable IDs are `src/main/features/core/settings/index.ts:154:32` (get), `:182:32` (set), `:173:31` (remove), and `src/renderer/store/auth.store.ts:52:29` (logout). The source uses Electron safeStorage/electron-store and credential-bearing renderer persistence; ADR-006's explicit-action/session-only differences remain deliberate. Source anchors were inspected in the immutable archive; this task claims scheduler behavior, not upstream authentication parity.

Integration should carry these new modules, their top-level Cargo test targets and this handoff together. Cargo/lib wiring is intentionally limited to storage; the primary P02.07 changes may need a small adjacent-hunk resolution there. Domain/cache/query code, existing browser probes, general CI and `STATUS.md` are untouched. No new dependencies, lockfile updates or native binaries are introduced.

Remaining work includes auth/settings consent and durable revocation reconciliation, production shell consumption, an asynchronous domain SecretStore adapter, provider-aware prompt handling, other providers and browser background scheduling. Browser remembering remains Unsupported, with the existing origin-storage policy unchanged. No live-keyring, production UI responsiveness, hard deadline, real-provider interruption or Linux packaging certification is claimed by this executor increment.

## Verification and independent review

The explicitly wired [portable tests](../../../tests/storage/credential_executor.rs) use non-Send deterministic doubles and fixed time samples; [native tests](../../../tests/storage/credential_worker.rs) use test-owned event gates and synthetic values. Neither uses a live keyring. Existing private-bus tests are separate from real provider certification. Windows/macOS builds and tests are excluded.

Rust/Cargo 1.92.0 used the [established launcher](README.md#reproduce-checks), with integration artifacts in a separate `CARGO_TARGET_DIR=/home/serhii/Projects/feishin-credential-executor-target`. Locked dependencies and Cargo.lock are unchanged. Checks passed:

- `cargo fmt --all -- --check`.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`.
- `cargo test --workspace --locked --offline`: 202 passed (including four compile-fail doctests), 13 existing optional tests ignored.
- Default and `linux-secret-service` executor targets: three dispatcher and nine worker tests passed for each selection after the final test fix.
- `cargo test -p feishin-storage --features linux-secret-service --test credential_executor --test credential_worker --test credentials --test linux_credentials --locked --offline`: 19 passed, one existing owned-GNOME test ignored. The two adapter cases check lazy construction and a test-owned private bus with no provider.
- `cargo clippy -p feishin-storage --features linux-secret-service --all-targets --locked --offline -- -D warnings`.
- `cargo clippy -p feishin-domain -p feishin-storage -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings`.
- `python3 tools/app/check-p00.py` (including maintained Markdown links) and `git diff --check`.
- In the unchanged local reference environment: `python3 tools/verify-inventory.py`, seven `python3 -m unittest discover -s tools -p 'test_*.py'` cases and `node --check tools/inventory-upstream.cjs`.

Root inspection found a race in the first shutdown fixture: releasing the provider before requesting shutdown allowed either expiry or cancellation. Shutdown now precedes release, with expiry exercised separately. Both feature selections and affected formatting/Clippy checks passed after the fix. The full workspace result predates that test-only ordering fix; the affected targets were rerun rather than repeating unrelated tests.

Tests exercise session-only defaults, explicit retention and failed operations, actual mutations before returned errors, missing replies after provider panic, generation correlation, size/admission limits, completed unread reply pressure, queued cancellation/expiry, late cancellation preserving success, shutdown with full unread replies and detached-drop cleanup. No owned GNOME daemon or browser runtime was rerun for this scheduling increment; the existing provider/browser evidence remains in [credential verification](credential-verification.md). No application packaging change or AppImage certification is claimed.

One independent Sol medium reviewer inspected all seven changed/new files and reported no actionable findings. The reviewer independently passed all 12 optional-feature executor tests and `git diff --check`. No additional fixes were requested. The review assessed the documented scope without claiming live-keyring, browser, UI responsiveness or hard provider-prompt deadline verification.
