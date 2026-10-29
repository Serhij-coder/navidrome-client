# ADR-005: portable models and local asynchronous contracts

Status: accepted for P02.01-P02.03 foundations, 2026-10-28. Backend discovery, protocol adapters and concrete task scheduling remain later implementation.

## Evidence and decision

The pinned Feishin `shared/types/domain-types.ts` defines normalized library entities, query filters and sort vocabulary, separate from backend DTOs and sort translation maps. `features-types.ts` and `shared/api/utils.ts:46-64` define sparse per-feature version sets with exact membership. The source queue identifies repeated songs by `_uniqueId`. The proposed architecture requires a single state owner, correlated completions, explicit units and browser futures without a universal `Send` bound.

Add one dependency-free `feishin-domain` crate. Models, typed IDs, units, catalogs, bounded owner example and service ports are modules inside it. Domain imports no platform/runtime/UI/HTTP/storage implementation. A second crate contains actual persistence DTOs/adapters; no speculative per-backend/engine/core crates are created. Existing P01 spike crates and shells remain experiments.

Distinct opaque IDs preserve backend strings. Standalone identities include server scope; related metadata inherits scope from its parent. Queue occurrences and playlist entries remain distinct from song IDs. Null, omission and empty values retain separate representations where the source declares them. Domain variant types replace `_itemType` strings. Optional backend `_custom` query bags and wire sort maps stay outside domain; this contract does not promise a backend implements every declared filter.

Use checked finite/nonnegative seconds-to-integer-millisecond/tick conversions, truncating fractional target units toward zero. Integer milliseconds are the selected portable precision for this foundation, derived from the normalizers' declared duration unit; sub-millisecond parity is unverified and must be characterized in adapter fixtures. Playback position, listened duration and signed lyric offsets are separate types. Protocol numeric ranges are checked by future adapters.

Local boxed futures permit Rc/browser handles and borrowed operation contexts. Native composition can use local tasks or adapter-specific Send boundaries; a single universal Send/Sync trait would exclude valid browser implementations. Every async boundary carries request/generation/deadline, cancellation and item/byte limits. Deterministic helpers and doubles prove the contracts; concrete adapters must enforce checks before/after I/O, task bounds and cancellation. Traits alone do not perform cancellation or provide a scheduler.

## Alternatives and consequences

A crate per architectural box would increase coordination without implementation. UI/wire DTO reuse would entangle portable state with backend/platform behavior. An untyped command bus would weaken identity and event validation. Domain errors are closed categories without raw credential-bearing strings; adapters own wire diagnostics/redaction.

The private bounded Owner is a concrete contract demonstration: loads are correlated, newer requests/local additions invalidate old replacements, duplicates and oversized/deadline/failure results leave the item snapshot unchanged. It is not P05 queue/shuffle/scrobble behavior, and no production playback state owner is claimed. The backend trait exposes normalized read conformance, not the full operation facade. Playback capability/event/command types are contracts, not an engine.

See [foundation rules and verification](../p02/foundation.md) and [migration map](../migration/p02-foundation.csv). Full cache keys, per-server cancellation/invalidation and serialization budgets beyond the implemented bounded fixtures remain P02.07. ADR-004 covers bounded persistence separately; historical credential/import policy remains open.

The subsequent [storage session/native worker](../p02/state-worker.md) implements concrete bounded storage scheduling only. It retains non-Send local stores, explicit correlation and owned review tokens. Its latest-start cancellation/deadlines do not implement in-flight backend/engine cancellation or change the general service-port obligations above.
