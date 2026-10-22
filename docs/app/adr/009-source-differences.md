# ADR-009: explicit compatibility treatment of source findings

Status: target contracts recorded for P00; no production implementation changed. Full runtime consequences remain pending where noted in [characterization](../baseline/characterization.md).

Preserve observed domain behavior unless a defect or isolation requirement justifies a documented difference. Distinguish isolated helper results from full app behavior. Regression inputs/results are in [source-audit.json](../baseline/source-audit.json), [wire-results.json](../baseline/wire-results.json) and [synthetic scenarios](../../../tests/fixtures/p00/server-scenarios.json).

| Cases | Decision for subsequent implementation | Regression / follow-up |
| --- | --- | --- |
| C01, C09 | Persist category-shaped settings correctly; retain all known legacy categories including `autoDJ`; apply the same validated migrated object shown in import preview. Preserve arrays by replacement. Never deliberately reproduce data loss. | [Headless traces](../baseline/headless-characterization.md) confirm stale category writes and legacy `autoDJ` omission. P02.04/P02.05 and P08.06 still require real exports, category corruption, columns, rollback and interruption coverage |
| C02 | Convert checked explicit units at adapter boundaries. Submission position must not multiply normalized milliseconds as seconds. | P03.06/P04.03/P06.06: wire fixtures for 180000 ms -> 180 seconds -> 1800000000 ticks; confirm original outgoing events |
| C03, C05, C06 | Preserve BOTH-as-OR, natural-end repeat semantics, occurrence identity and orphan cleanup. | P05/P06 deterministic traces, including manual Next versus ended and duplicate occurrences |
| C04 | Represent unknown version/capability explicitly; discover extensions instead of granting every optional feature on an unparseable version. This intentionally tightens the source fallback. | P03/P04: malformed/dev versions, failed extension probe and supported base fallback; no family-wide optional support assumption |
| C07 | Scope lyric/artwork/query caches by server plus resource dimensions. Intentional isolation improvement. | P02.07/P09: same song ID on two servers, in-flight switch and eviction |
| C08 | Cancel or reject obsolete Auto DJ refills by generation and coalesce bounded work. Intentional race prevention. | P05.06/P05.08: delayed responses during skip, clear, server and settings changes |
| Listen speed | Preserve media-position accumulation; do not silently switch to wall time. | Controlled exact-callback speed traces now exist; actual audio clock/engine behavior and P06.06 integration remain unverified |

Alternatives include copying suspected defects or making undocumented fixes. Neither gives a usable compatibility contract. These decisions do not turn partial source characterization into a passed runtime gate, and do not claim any feature has been ported.

The new last-item stop trace emits the first occurrence's ID after index reset. It is recorded as an unresolved event-consumer ambiguity, not an accepted behavior change; investigate engine/reporting consumers before adding a compatibility decision.
