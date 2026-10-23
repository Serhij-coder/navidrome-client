# Development workflow

[AGENTS.md](../../AGENTS.md) is the canonical policy for task ownership, delegation, verification, review and delivery. Project defaults live in [.codex/config.toml](../../.codex/config.toml); concise [role files](../../.codex/agents/) select the delegate models and instructions. Personal global settings are unchanged.

## Setup and activation

Use a Codex client that supports project configuration, standalone custom agent TOMLs and the requested models. Open this repository as the project and review its configuration before trusting it through the client's project-trust controls. Project configuration is loaded only for trusted projects. Restart or start a fresh session after changing configuration or role files so the client can discover them. Existing running agents retain their session settings; adding these files does not change the current orchestrator's model.

The project requests `gpt-6.1-sol` with `high` reasoning for the orchestrator. Delegate defaults are `gpt-6.1-sol` with `low` reasoning. Named roles are:

| Role | Model | Reasoning | Responsibility |
| --- | --- | --- | --- |
| `implementer` | `gpt-6.1-sol` | `low` | Bounded implementation and affected checks |
| `implementer-medium` | `gpt-6.1-sol` | `medium` | Automatic escalation scheduled by the orchestrator |
| `researcher` | `gpt-6-luna` | `xhigh` | Read-only exploration and evidence gathering |
| `reviewer-correctness` | `gpt-6.1-sol` | `medium` | Correctness, compatibility and architecture review |
| `reviewer-operations` | `gpt-6.1-sol` | `medium` | Tests, edge cases, performance and operational review |

## Coordination and delivery

The orchestrator decomposes tasks, makes architecture decisions, assigns exclusive file ownership, chooses shared checkout or worktrees and allocates runtime slots. Implementers may spawn researchers within that allocation. Researchers and reviewers never delegate. When the slot budget is exhausted, return findings or request scheduling; do not recursively wait for unavailable slots. Finish implementation/research agents before starting the two independent reviews. The concurrency setting is deliberately unset so the orchestrator can adapt to the client's actual limit.

Implementation proceeds through affected checks, fixes and passing reruns, then exactly two independent reviews. Resolve findings, rerun affected checks and obtain focused re-review for material fixes before delivery. Each assignment includes task IDs, upstream anchors, acceptance criteria and allowed files. Implementers hand back changed files, exact checks/results, unresolved risks and integration needs; the orchestrator runs combined integration checks after assembly. Completion of an assignment does not establish a phase exit. The orchestrator owns integration checks, coordination records and commit/push. Delegates do not commit or push. See the complete acceptance and delivery requirements in [AGENTS.md](../../AGENTS.md).

## Client limitations

Model access, named roles, project trust and runtime limits depend on the client and account. For a client without named roles, explicitly spawn with the model and reasoning effort in the table and supply the matching role instructions. Model/effort overrides may require a fresh child context rather than an inherited full-history fork; include the assignment, canonical rules, evidence and acceptance criteria explicitly in that case. Do not silently substitute an unavailable model or effort. Report missing capabilities and leave the task uncommitted if a required check or reviewer gate cannot run.

Configuration parsing and TOML checks establish syntax and schema compatibility only. They do not prove that a client has activated the roles, granted model access or changed an existing session. Verify the selected model/effort in the client's agent/session details when available. No project permission or sandbox overrides are configured.

Codex documentation: [subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents), [configuration](https://learn.chatgpt.com/docs/config-file/config-basic), [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

On 2026-10-23, installed Codex 0.163.0 passed an app-server `--strict-config` initialization and `config/read` smoke check with this repository as `cwd`: the project layer was active, root model/effort were `gpt-6.1-sol`/`high`, and agent defaults were enabled with `gpt-6.1-sol`/`low`. TOML parsing and required role keys also passed. No model inference was requested; named-role discovery and runtime spawning remain unverified.
