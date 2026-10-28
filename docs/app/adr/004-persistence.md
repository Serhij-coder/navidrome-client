# ADR-004: bounded development profile persistence

Status: accepted for P02.04 non-secret development profiles only, 2026-10-28. Historical import/rollback and credential policy decisions remain open under P02.05/P02.06.

## Evidence and decision

Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` persists credential-bearing authentication state (`auth.store.ts:25-118`) and split settings (`store/utils.ts:178-325`). The settings defaults include `language: 'en'` and `followSystemTheme: false` (`settings.store.ts:1374,1360`). Current Rust shells have no durable application state. We need independent development persistence before authentication or queue integration, without reading the Electron profile.

Use one small `feishin-storage` crate beside the dependency-free domain crate. Versioned storage DTOs are distinct from domain values; persisted profile IDs are strings validated for bounded nonempty uniqueness, and backend kind is a closed schema enum. Future application composition converts them to domain ServerId/ServerType. Schema v1 contains at most 64 non-secret server descriptors (ID, name, HTTP(S) endpoint, backend), an optional selected server ID and two preferences. It intentionally does not persist the complete domain ServerProfile or settings surface. No credentials, runtime handles, queue or cache belong to this schema.

The native blocking adapter takes an explicit absolute development root and writes only its `feishin-rust-dev-p02/profiles-v1.json` child. It rejects symlink components and parent traversal; callers should pass a canonical root on platforms with symlinked temporary directories. The root is trusted, single-owner application data; these path checks do not promise resistance to a hostile concurrent directory replacement. No path is inferred from Electron settings. Run native operations on a bounded filesystem worker, never the UI/async executor. This increment provides the blocking worker boundary, not its production executor integration.

A same-directory temporary file is flushed and synced before atomic replacement; Unix also syncs the directory. Errors after replacement may mean the new value is visible with uncertain durability and are reported. Cross-process writers and power-loss certification on every filesystem/OS are outside this increment. Native tests check replacement/reopen, unchanged original fixture, ignored interrupted temporary data and schema preservation.

The browser adapter replaces one origin-scoped localStorage value under `feishin-rust:dev:p02:profiles:v1`. One-key replacement avoids partial multi-key category writes. Quota/security/disabled storage failures remain errors. The 256 KiB UTF-8 document bound limits serialization and parsing; localStorage retrieval itself necessarily allocates the browser-provided string. Multiple-tab last-writer coordination and larger async IndexedDB stores require follow-up before application integration.

## Alternatives and consequences

SQLite/IndexedDB provide transactions and scale better for queue/cache data but add unnecessary machinery for this bounded profile schema. A shared key with the original app would violate isolation. Multiple split preference keys would need a transaction strategy; the source split-key writer ambiguity remains unmodified and will be characterized in P02.05.

Reads are side-effect-free. Missing fields use documented defaults. Invalid profile records are rejected individually; duplicate IDs keep the first valid profile; an orphan selected ID is cleared. Invalid preference fields default independently with typed recovery diagnostics. Invalid JSON, invalid envelopes and unsupported schema versions fail explicitly and are protected from overwrite by repository saves. Unknown envelope/preferences fields are rejected rather than silently erased. A valid partial-recovery result may be explicitly saved by the caller; no automatic repair write occurs. Whole-document recovery currently requires manually preserving/quarantining the damaged value before starting a new profile; automated backup/rollback is P02.05.

Endpoint validation rejects URL userinfo, queries and fragments while allowing intentional HTTP LAN servers and base paths. This is a structural non-secret policy, not a claim to detect arbitrary secrets placed in user text or URL paths. Credentials are excluded by schema and no credential-remembering implementation is selected. Native/browser adapters are not connected to a shell yet.

See [P02 implementation and checks](../p02/README.md) and [migration evidence](../migration/p02-storage.csv). This ADR does not establish full P02 completion or credential/import compatibility.
