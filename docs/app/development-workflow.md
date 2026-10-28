# Development workflow

[AGENTS.md](../../AGENTS.md) is the canonical policy for task ownership, delegation, verification, review and delivery. Project defaults live in [.codex/config.toml](../../.codex/config.toml); concise [role files](../../.codex/agents/) select the delegate models and instructions. Personal global settings are unchanged.

## Setup and activation

Use a Codex client that supports project configuration, standalone custom agent TOMLs and the requested models. Open this repository as the project and review its configuration before trusting it through the client's project-trust controls. Project configuration is loaded only for trusted projects. Restart or start a fresh session after changing configuration or role files so the client can discover them. Existing running agents retain their session settings; adding these files does not change the current orchestrator's model.

The project requests `gpt-6.1-sol` with `high` reasoning for the orchestrator. Delegate defaults are `gpt-6.1-terra` with `medium` reasoning. Named roles are:

| Role | Model | Reasoning | Responsibility |
| --- | --- | --- | --- |
| `implementer` | `gpt-6.1-terra` | `medium` | Bounded implementation and affected checks |
| `implementer-medium` | `gpt-6.1-sol` | `medium` | Sol escalation scheduled by the orchestrator for difficult Terra work |
| `researcher` | `gpt-6-luna` | `high` | Read-only routine narrow exploration and evidence gathering |
| `reviewer-correctness` | `gpt-6.1-sol` | `medium` | Correctness, compatibility, architecture, tests, edge cases, performance and operations review |

## Coordination and delivery

The orchestrator defaults to one implementer and parallelizes substantial independent work only. It assigns exclusive file ownership, chooses shared checkout or worktrees and allocates runtime slots. It schedules research; researchers and reviewers never delegate. For complex evidence work, it may explicitly spawn a fresh default-role child with `gpt-6-luna` and `xhigh` plus the researcher instructions. The named `researcher` role fixes `high`, so do not use it for that exception. Finish implementation/research agents before the final independent review. Concurrency is deliberately unset so the orchestrator can adapt to the client's actual limit.

Use fresh delegate contexts with compact task packets: scope, acceptance criteria, allowed files, task IDs, relevant excerpts and upstream anchors. The orchestrator reads task-relevant reference sections for application behavior or architecture changes; workflow-only tasks do not require rereading unchanged general references. Delegates read canonical rules, the scratchpad and task-specific documents, including the exact baseline for behavior work; avoid repeated general reads. Trust exploration while verifying material integration assumptions. Implementers report changed files, exact checks/results, unresolved risks and integration needs; the orchestrator runs combined integration checks. An assignment does not establish a phase exit.

Batch a coherent feature's implementation, tests, documentation and migration evidence before one review and commit. Do not create separate reviewed documentation commits solely for unchanged CI results. Run affected checks, fix failures and rerun affected checks until passing, then obtain one independent combined review. Resolve findings, rerun affected checks and obtain focused re-review for material fixes. Preflight the stable toolchain/environment and reuse an established documented launcher. Do not repeat a full suite without justification. Use scripted event-based observation of CI/tests with bounded output, not busy model polling, and provide progress updates at most 60 seconds apart. The orchestrator owns integration, records and commit/push; delegates do not commit or push. See [AGENTS.md](../../AGENTS.md) for delivery requirements.

Evaluate this configuration over three accepted tasks using total task cost, rework and defects; record the results before drawing conclusions. No savings are assumed.

## Client limitations

Model access, named roles, project trust and runtime limits depend on the client and account. For a client without named roles, explicitly spawn with the model and reasoning effort in the table and supply the matching role instructions. Model/effort overrides may require a fresh child context rather than an inherited full-history fork; include the assignment, canonical rules, evidence and acceptance criteria explicitly in that case. Do not silently substitute an unavailable model or effort. Report missing capabilities and leave the task uncommitted if a required check or reviewer gate cannot run.

Configuration parsing and TOML checks establish syntax and schema compatibility only. They do not prove that a client has activated the roles, granted model access or changed an existing session. Verify the selected model/effort in the client's agent/session details when available. No project permission or sandbox overrides are configured.

Codex documentation: [subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents), [configuration](https://learn.chatgpt.com/docs/config-file/config-basic), [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

Historical validation of the previous configuration: on 2026-10-23, installed Codex 0.163.0 passed an app-server `--strict-config` initialization and `config/read` smoke check with this repository as `cwd`: the project layer was active, root model/effort were `gpt-6.1-sol`/`high`, and agent defaults were enabled with `gpt-6.1-sol`/`low`. TOML parsing and required role keys also passed. This predates the Terra and single-review configuration and does not validate it. No model inference was requested; named-role discovery and runtime spawning remain unverified.

On 2026-10-28, Codex CLI 0.164.0 passed strict app-server initialization and `config/read` with this repository as `cwd`: effective root model/effort were `gpt-6.1-sol`/`high`, and enabled delegate defaults were `gpt-6.1-terra`/`medium`. A fresh child explicitly using Terra medium performed this workflow edit. This establishes explicit model/effort spawning, not automatic named-role discovery or quality equivalence on application tasks.
