# ADR-007: bounded server-scoped query cache foundation

Status: accepted for bounded P02.07 domain/library behavior, 2026-10-29. Backend/UI lifecycle wiring, full endpoint coverage and transport/queue serialization evidence remain open. No P02 exit is established.

## Evidence and identity

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` defines key factories in `src/renderer/api/query-keys.ts`. `splitPaginatedQuery` at `30:36` partitions filters and pagination; song count (`378:16`), infinite (`397:23`) and list (`409:15`) all retain pagination when provided. Count keys must not silently discard it. The shared QueryClient uses 10-second stale time and 20-second unused-query garbage collection (`src/renderer/lib/react-query.ts:13-40`). Some global player/remote-lyrics keys omit server IDs; their intentional sharing was not established by this static inspection.

Keep one dependency-free domain module. Keys encode every field of the seven currently modeled normalized list queries, list/count/infinite kind, server and an explicit selected-library context. Detail keys encode item kind, ID, server and library scope, but do not represent additional endpoint-specific detail parameters. Missing and empty options/vectors differ; vector order is preserved because equivalent-set semantics have not been characterized. Backend custom parameters and unmodeled request dimensions must extend the typed schema or bypass this cache, never reuse an incomplete key.

Use an internal schema-v1 bounded binary identity, not backend wire DTOs or a new cache database. Validate borrowed lengths/counts before copying; reject invalid/oversized keys without truncation. Key/ticket Debug expose only bounded metadata, and diagnostics contain no IDs, queries or values. Internal canonical bytes are not an authorization token or suitable raw telemetry. Caller-provided cache instance IDs must be nonzero and unique while tickets from a retired instance can still exist.

## Lifecycle and budgets

`QueryCache<V>` accepts non-Send/non-Debug/non-Clone values. Producers supply accounted normalized-payload bytes/items; this is not generic heap introspection or a guarantee about preallocation in transport/normalization. Limits bound registered servers, retained entries, pending requests and accounted values. Key copies have a separate fixed per-key cap/count bound. Rejected value updates preserve existing cached data. Deterministic least-recently-used order controls eviction.

Choose a bounded owner-driven idle policy: values are fresh for 10 seconds and expire after 20 seconds without a cache access/insertion. This intentionally differs from TanStack's observer-based unused-query GC. No observer tracking, retry/refetch, infinite-page aggregation, optimistic mutation or persisted/offline cache is implemented. Those belong to concrete consumers/adapters and require their own evidence.

Tickets carry exact key, instance, request serial, server generation, deadline and a cooperative cancellation token; their context maps to the existing typed domain RequestContext. New matching work supersedes old work. Exact-ticket/server cancellation, server/family invalidation and server retirement reject late completions. Errors and expired/cancelled results never populate the cache; no mutation retry occurs. Drop marks pending tickets cancelled. These flags do not abort an HTTP socket or force completion of another runtime's work.

Callers must bind logout, profile/permission/endpoint changes, server selection, library selection and successful mutations to the appropriate cancellation/invalidation. Source logout cancels globally before deferred clear, folder changes clear globally, and mutations invalidate distributed prefixes. The new library supplies scoped mechanisms without claiming those UI handlers are ported. `cancel_server` retains existing data; authentication/context changes needing data removal must invalidate/remove instead. Detail keys map to the corresponding entity family for supported list families; remaining generic item families require explicit broader invalidation.

Source behavior is static evidence; target budgets, idle policy, explicit library context and universally server-scoped modeled keys are architectural choices. [Migration records](../migration/p02-cache.csv) distinguish these replacements from deferred feature keys/call sites. The existing bounded imported-queue serializer remains separate from full P05 queue/session transport serialization.

See [contract](../p02/query-cache.md), [verification](../p02/query-cache-verification.md) and [ADR-005](005-portable-contracts.md). The parallel credential executor is integrated separately and preserves its [own documented scope](../p02/credential-executor.md).

The bounded [payload codec continuation](015-bounded-payloads.md) derives exact byte/item Weight for encoded SongSummary pages and retained queue snapshots. Generic cache values still rely on trusted producer accounting; this does not add backend/remote wire identity or transport allocation guarantees.
