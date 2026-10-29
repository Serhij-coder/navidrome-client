# Feishin Rust project rules

## Purpose and current state

This project is a Rust rewrite of [jeffvli/feishin](https://github.com/jeffvli/feishin), initially pinned to `7d6ad318c79802e226996cfc39acba6b05bb22e8` (development, package 1.17.0). Preserve the agreed desktop, browser and remote-controller behavior. The workspace currently contains analysis/planning tools and documents, not a Rust application.

The orchestrator reads the brief [project scratchpad](docs/app/STATUS.md), [README.md](README.md) and relevant tracked [application documentation](docs/app/README.md) at the start of every task. Delegates read this canonical policy, the scratchpad and task-specific required documents; use supplied relevant excerpts and anchors instead of repeatedly reading unchanged general documents. Verify facts that matter to the task; the scratchpad is a handoff aid, not proof of completion. When the local rewrite reference package is available, the orchestrator reads the task-relevant sections of `docs/IMPLEMENTATION-LOGIC.md`, `docs/RUST-ARCHITECTURE.md` and `docs/RUST-REWRITE-PLAN.md` before implementing application behavior or architecture; unchanged general reference material need not be reread for workflow-only tasks. Delegates read task-specific reference evidence and the exact baseline before behavior work. Treat proposed architecture choices as proposals until their phase gate/ADR is complete. A clean checkout must not require ignored files just to build, test or understand the implemented app; do not invent missing source evidence for a migration task that requires it.

The user selected egui for the native desktop UI and prioritizes desktop performance. Appearance, layout and widgets may be redesigned; preserve agreed features, effective workflows and accessibility. The local reference `docs/adr/001-egui-desktop.md` records this direction; maintain implemented app decisions and measured performance gates in tracked app documentation. This direction does not establish runtime feasibility or remove browser/remote deliverables.

## Repository contents and maintenance

- Keep the current rewrite plan, analysis documents, inventory, upstream archive and analysis scripts local and ignored. `.gitignore` excludes `docs/*` and `tools/*`, with exceptions for `docs/app/` and `tools/app/`. Do not force-add ignored reference material to an application commit.
- Maintain the app's own documentation in `docs/app/` and tooling in `tools/app/` alongside implementation. Update build/run/test instructions, configuration, supported capabilities, ADRs, migration records, benchmarks, packaging and troubleshooting when relevant behavior changes.
- Keep application test code and fixtures in the dedicated top-level `tests/` directory, organized by crate/feature and test kind. This includes tests for app tooling; `tools/app/` holds automation, not the test suite. Wire test files explicitly into the appropriate Cargo package/test harness and CI; a workspace-root test directory is not automatically discovered. If private internals need unit coverage, include the corresponding test files from `tests/` under the owning crate's `cfg(test)` boundary rather than widening production visibility. Existing ignored analysis-tool tests remain local reference checks, not application tests.
- Keep `docs/app/STATUS.md` as a brief scratchpad, at most 25 non-empty lines. Update it after meaningful progress, task completion or a blocker change so the next agent can quickly see the implemented state/current phase, active task, last relevant checks, blockers and next action. Replace stale entries; use short bullets and links to detailed records instead of accumulating history, logs or repeated design explanations. Distinguish planned work from verified behavior and exclude secrets.
- App builds and CI must work from tracked inputs without local planning evidence or analysis tools. Before a migration task that needs unavailable upstream fixtures/source, report the missing evidence rather than pretending parity is verified; continue independent work where possible.
- Use the root GNU GPL version 3 [LICENSE](LICENSE), preserve Feishin attribution, and maintain third-party notices for reused code/assets and distributed dependencies. Include the project license and required notices in release artifacts.
- If reference files were already tracked, ignore rules alone do not untrack them. When a working Git index is available, remove only those reference paths from the index with `git rm --cached` (using `-r` for directories), keeping local files intact. Do not remove tracked app docs/tools or unrelated changes.

## Scope and traceability

- Use task IDs from the rewrite plan and stable upstream IDs from `docs/inventory/functions.tsv`. Record the upstream source, target symbol, behavior and verification for every migrated area.
- Keep `docs/inventory` and the archived source as immutable local baseline evidence. Maintain rewrite progress in a separate tracked migration map under `docs/app/migration/`; do not change `portStatus` in the baseline to claim progress.
- Do not silently omit advanced features, platforms, backend differences, assets or configuration. Record `ported`, `replaced`, `retained`, `unused` or `deferred` with evidence and a reason. Deferral is not full-parity completion.
- Port observable behavior and domain rules. Rust does not need to reproduce React hooks, Electron IPC names or every callback one for one.
- Distinguish observed source behavior, inferred intent, a proposal, and a verified runtime result. A static inventory does not prove correctness or reachability.
- Characterize suspected upstream defects before changing behavior. Record deliberate differences in an ADR and add a regression fixture.
- Do not execute upstream installers, package lifecycle hooks or reference scripts just to inspect source. Use a controlled reference environment when runtime characterization is needed.

## Architecture boundaries

- Keep domain models/reducers portable. Domain/core must not import UI, browser, filesystem, HTTP, OS or concrete database/process code.
- Put wire DTOs, authentication, envelope errors, backend quirks and normalization inside backend adapters. UI consumes normalized domain models through the application boundary.
- Keep Navidrome native and Subsonic delegation explicit. Never assume all servers of one family expose every optional extension.
- Use one owner for playback/queue state. UI, engines, remote and OS integrations send typed commands; they must not independently mutate the queue.
- Use distinct IDs for servers, songs, queue occurrences, playlist entries and devices. Preserve repeated occurrences of the same track.
- Use explicit time units and checked conversion helpers for seconds, milliseconds and Jellyfin ticks. Keep playback position separate from listened duration.
- Attach generation/request IDs to asynchronous work. Cancel or reject stale completions; bound task queues and operations with deadlines.
- Keep native and WASM implementations behind explicit boundaries. Do not introduce desktop-only dependencies into portable core or assume browser futures are `Send`.
- Record major toolkit, protocol, persistence, engine and deployment choices under `docs/app/adr/` with alternatives, evidence and consequences. Add crates only when the feature needs the boundary.

## Rust implementation standards

- Pin the toolchain/MSRV and commit the application Cargo lockfile once P01 creates the workspace. Avoid dependency upgrades unrelated to the active task.
- Prefer typed enums/newtypes, `Result`/`Option`, small cohesive modules and clear ownership. Avoid stringly typed command buses and global mutable state.
- Do not use `unwrap`/`expect` for recoverable user/network/storage/device inputs. Internal invariants may use assertions with an explanation and tests.
- Keep `unsafe` and FFI isolated, minimal and documented with safety invariants. Prefer safe interfaces; test native resource ownership/cleanup.
- Never block the UI/async executor with decoding, filesystem scans or synchronous networking. Use bounded workers and cancellation for expensive tasks.
- Do not hold a lock across an await. Do not swallow errors or turn failed mutations into success/null without a documented compatibility reason.
- Use a standard formatter and Clippy. Favor plain names and direct code over speculative abstraction or premature crate fragmentation.

## Packaging and distribution

- Every Linux desktop release must include an executable `.AppImage` for each supported Linux CPU architecture. AppImage packaging is a required deliverable.
- The AppImage must launch without a development checkout or Rust toolchain. Bundle required assets/runtime libraries and explicitly check/document any external mpv/FFmpeg requirements.
- Build AppImages in CI and smoke-test them on clean environments from the supported Linux matrix. Verify application startup, resource loading and playback before marking Linux packaging complete.

## Data, privacy and compatibility

- Develop with a separate application data directory. Import copies/exports from Feishin; never migrate its live profile in place by default.
- Version persisted schemas and test migrations, partial corruption, recovery and rollback. Serialize durable preferences/session data only; exclude sockets, handles, timers and listeners.
- Separate secrets from non-secret profiles where the platform permits. Remember credentials only according to explicit settings. Never print raw credentials, authorization headers or authenticated stream URLs.
- Keep cache keys server-scoped and include filter/library/page dimensions. Prevent cross-server reuse of same-ID song/artwork/lyric data.
- Gate optional features by protocol versions, permissions and engine/platform capabilities. Unsupported behavior must be explicit.
- Preserve private mode across reporting/diagnostic/remote integrations. Do not add external analytics without an explicit recorded product policy.
- Validate external URLs, file/path inputs, remote messages and proxy destinations at their service boundaries. Preserve intentional LAN music-server access without introducing an arbitrary unauthenticated proxy.
- Preview local tag edits and preserve unrelated metadata/audio. Report per-file failures; do not claim transactional batch writes unless implemented and tested.
- Preserve upstream license and attribution for reused source/assets. Keep third-party/native-binary notices with release artifacts; do not present copied implementation as newly authored.

## Verification

Before a Cargo workspace exists, validate affected Markdown links, ignore rules and other changed artifacts. When the local reference package is present, run its applicable checks below; these ignored tools are not dependencies of application builds or CI:

```sh
python3 tools/verify-inventory.py
python3 -m unittest discover -s tools -p 'test_*.py'
node --check tools/inventory-upstream.cjs
```

After P01, use applicable checks (these are intended commands, not claims that a workspace exists today):

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run target-specific native/WASM checks using the crate names/features established in P01; do not blindly enable mutually exclusive platform features with `--all-features`.

Current maintainer-directed build/test scope is Linux and WASM/browser. Exclude Windows and macOS build/test runs; retain historical evidence and platform code without claiming current verification. Linux x86_64/aarch64 AppImage checks remain required.

- Test queue/scrobble reducers using fixed clocks/RNG and meaningful generated/property/differential traces.
- Use redacted backend fixtures for success, malformed/optional data, protocol errors, cancellation and unsupported capabilities. Test mutations against disposable fixtures/resources.
- Test persistence using historical/corrupt serialized inputs and interruption recovery, not only current-schema round trips.
- Test engine teardown, late callbacks, process death and reconnect. Hardware/browser/platform behavior needs corresponding integration/manual evidence.
- Add a regression test when fixing behavior. Avoid tests that merely repeat implementation details or unnecessary broad test reruns for small documentation changes.
- Whenever the primary agent or a reviewer finds a mistake/error, fix it and rerun the test that exposed it (when applicable), relevant regression tests and affected checks. Earlier passing results do not validate a later fix. Repeat fix -> test until the affected checks pass; this applies to fixes made before, during or after review, including mistakes found through inspection or manual testing.
- Report exactly what ran, what passed/failed, and what was not available. Never substitute static checks for audio/device/end-to-end parity.

## Agent roles and coordination

- Use the project configuration in [.codex/config.toml](.codex/config.toml) and concise role files under [.codex/agents/](.codex/agents/). This file remains the canonical policy; see [development workflow](docs/app/development-workflow.md) for setup and client limitations. Do not change personal global settings.
- The orchestrator uses `gpt-6.1-sol` with `high` reasoning. It owns task decomposition, architecture decisions, integration checks, coordination records and all commit/push operations. It delegates implementation and retains responsibility for the complete result.
- Standing authorization permits bounded implementation and read-only research. The default implementer uses `gpt-6.1-terra` with `medium` reasoning. The orchestrator may escalate difficult implementation to `implementer-medium` (`gpt-6.1-sol`, `medium`), never higher without a new user decision.
- Researchers use `gpt-6-luna` with `high` reasoning for routine narrow exploration. For complex evidence work the orchestrator may explicitly schedule `gpt-6-luna` with `xhigh` in a fresh default-role child with researcher instructions; the named `researcher` role fixes its effort to `high` and must not override that assignment. Researchers are read-only and do not delegate; the orchestrator schedules research.
- Default to one implementer; parallelize substantial independent work only. The orchestrator chooses a shared checkout or isolated worktrees, concurrency and exclusive file ownership before concurrent work. Use fresh delegate contexts and compact task packets with scope, acceptance criteria, allowed files, task IDs, relevant excerpts and upstream anchors rather than full parent history. Require handoffs with changed files, exact checks/results, unresolved risks and integration needs. Trust delegated exploration while verifying material integration assumptions. Run combined integration checks after assembling delegated changes; an assignment does not establish a phase exit. Preserve unrelated changes and coordinate integration centrally.
- Balance elapsed time and token cost without reducing quality or bypassing checks. Respect the runtime slot limit; do not hard-code a concurrency count in project configuration. Finish implementation/research agents before the final review.
- One independent `reviewer-correctness` reviewer uses `gpt-6.1-sol` with `medium` reasoning and covers correctness, architecture, compatibility, tests, edge cases, performance and operations. The reviewer is read-only and must not delegate, commit or push. No delegate may commit or push.
- If named roles are unavailable, explicitly supply the same model and reasoning effort when spawning, plus the role instructions. Do not silently substitute models or reasoning effort, or bypass an unavailable required reviewer/check gate. Report unavailable capabilities to the orchestrator; leave unfinished work uncommitted.

## Workflow and completion

- The orchestrator owns the full pipeline below. Bounded delegate assignments are parts of that task, not separate coherent tasks requiring recursive reviews or commits; researchers and reviewers only inspect and report.
- Follow this development pipeline for every coherent task, including documentation, tooling and project-rule changes: **Code implementation -> tests -> fixes (if needed) -> rerun affected tests until passing -> one independent subagent with clear context to review changes -> fixes (if needed) -> rerun affected tests until passing -> commit/push**. Reruns are required whenever a fix changes what was tested; unchanged passing checks need not be repeated. For documentation-only tasks, implementation means the requested document changes and tests mean applicable documentation checks (plus local inventory checks when that reference package is present).
- Run applicable checks after implementation; fix failures and rerun affected checks before review. Add meaningful behavior/regression tests where appropriate under the verification rules above; do not add artificial tests solely to satisfy the pipeline.
- After checks pass, assign one independent review subagent to the task changes. This is standing authorization for that review subagent, using the reviewer model and effort above. The reviewer inspects and reports without editing, committing, pushing or recursively delegating the review.
- Give the reviewer clear context: the user request, relevant task IDs and acceptance criteria, applicable project rules/ADRs, changed files and exact diff or before/after snapshots, relevant upstream anchors, checks/results and known limitations. The reviewer inspects the actual changes for correctness, compatibility, architecture, tests, edge cases, performance and operational risks, and returns actionable findings with file/line references and severity, or explicitly states that no actionable findings were found.
- Collect the review before committing. Resolve actionable findings, record the reason for any finding judged inapplicable, and rerun affected tests/checks after every fix before considering the finding resolved. Send material fixes back to the reviewer for focused re-review so the final change scope is covered; any further fixes repeat the fix -> test loop. Do not commit/push until final fixes have passing affected checks and the reviewer is satisfied. If the reviewer or a required check is unavailable, report the incomplete gate and leave the task uncommitted.
- Batch a coherent feature's implementation, tests, documentation and migration evidence for one review and commit; do not create separate reviewed documentation commits solely to record unchanged CI results. Preserve required task commits and honest evidence. Preflight the stable toolchain and environment, reuse an established documented launcher, and avoid repeating a full suite without justification. Observe CI and tests with scripted event-based waits and bounded output rather than busy model polling; provide progress updates at most 60 seconds apart.
- Inspect existing work before editing; preserve unrelated user changes. Keep patches focused on the active task and update the relevant plan/progress record.
- Use `rg` for code searches. Consult the exact baseline implementation before guessing backend behavior.
- Continue routine authorized work without repeated approval requests. Ask for missing product decisions only when they block or materially change the task; progress independent work meanwhile.
- The orchestrator must automatically create a local Git commit after each coherent, completed task once applicable checks and the subagent review are satisfied, then push that task commit to the current branch's configured upstream. This is standing authorization for the orchestrator to commit and push task changes without asking again, including documentation and project-rule updates. Do not wait for an entire rewrite phase to finish before committing a completed task.
- Inspect Git status and diffs before staging; stage only the task's intended files/hunks and preserve unrelated or pre-existing user changes. Review the staged diff for accidental files and secrets before committing. Never store real credentials in commits, fixtures, logs or docs.
- Use concise commit messages describing the resulting change; include rewrite task IDs when applicable. Keep commits focused and include relevant tests, documentation and progress records. A commit does not establish phase completion without the required exit evidence.
- Create new commits by default. Do not amend existing commits, rewrite history, discard changes or bypass hooks/checks unless explicitly authorized. If verification fails, fix it before committing; if blocked or unavailable, report the limitation and leave the unfinished changes uncommitted.
- If Git metadata, repository access or author identity is unavailable, report that the changes could not be committed. Do not invent an author identity or replace repository metadata to force a commit.
- Before pushing, inspect the outgoing commit range and destination. Push only the authorized task commits with a normal fast-forward push; do not force-push, switch branches, guess/create a remote or include unrelated pre-existing commits. If there is no configured upstream, authentication/access fails or the remote has diverged, retain the local commit and report the push blocker. Do not rewrite history to force delivery.
- Publishing releases/artifacts, deployment and sending messages still require separate authorization; automatic Git commit/push does not authorize these actions.
- For completed changes, report outcome, relevant files, verification, the review outcome and fixes, commit hash and push destination/status (or why either step was unavailable), and remaining material gaps. Do not claim phase completion without its exit evidence.
- Use plain technical English and ASCII dashes in authored prose. Exact upstream code remains unchanged in evidence artifacts.

`AGENTS.md` is the single canonical project rules file. Do not create a duplicate `agent.md`.
