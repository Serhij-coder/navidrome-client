# Project scratchpad

- Stage: bounded P02 models/storage/imports/preferences/credentials/cache foundations. P00/P01/P02 exits open; not a working music client.
- Sequencing: [P02 exception](p01/exit-decision.md) keeps P01 open; address only concrete implementation blockers. No release/parity waiver.
- Scope: Linux/WASM/browser checks; Windows/macOS excluded. Both Linux AppImage CPUs retained.
- P02.07: seven normalized list-key factories and scoped detail identity; server/library/filter/page separation, bounded schema-v1 keys, redacted Debug.
- Cache: deterministic bounded LRU; 10s freshness/20s idle expiry; producer-accounted value budgets, fixed counts-only diagnostics. No heap/transport allocation guarantee.
- Async: exact request/instance/server-generation tickets, deadline/supersession/cancel/invalidation/retirement rejection; Drop cancels pending flags. No HTTP abortion claim.
- Credential executor: integrated branch9ab58b5; typed local dispatcher and bounded native worker; provider constructed/called on worker, no universal Send bound.
- Credential limits: session-only default; explicit remember/restore/forget. Uncertain mutations preserved; latest-start deadline only, prompt/in-flight calls not interruptible.
- Storage: isolated versioned profiles/import journal/active-state repository and bounded worker; opt-in desktop two-field preferences. Full shell/auth/import adoption open.
- Checks: assembled workspace213 pass incl4 compile-fail/13 optional ignored; workspace fmt/Clippy and domain/storage/web/remote WASM Clippy pass.
- Optional credential-feature Clippy and12 executor/worker cases pass. Cache WASM runtime: Chromium141/Firefox142/WebKit26 each4 assertions pass, stores unchanged.
- [Cache contract](p02/query-cache.md), [ADR007](adr/007-query-cache.md), [migration](migration/p02-cache.csv), [verification](p02/query-cache-verification.md).
- [Credential executor](p02/credential-executor.md) separately reviewed; branch CI all5 jobs passed. Combined independent Sol medium review satisfied; no actionable issues.
- Gaps: P02.05 complete imports/adoption; P02.06 auth/consent/revocation/prompt integration; P02.07 endpoint coverage, transport/UI cancellation and runtime queue/query budgets.
- Next: after hosted CI, bound runtime query/queue serialization and diagnostic consumers; keep P03 on its separate branch pending integration decision.
- P01 [remaining gates](p01/README.md) stay open; no unrelated prototype expansion or production reducer/engine claims.
- Workflow: Sol high orchestration, Terra medium implementation, one final independent Sol medium review. [Setup](development-workflow.md).
