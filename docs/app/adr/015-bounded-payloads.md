# ADR-015: bounded song-page and retained-queue payloads

Status: accepted for bounded P02.07 payload codecs, 2026-10-29. No remote protocol or phase exit is selected.

## Evidence

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` serializes remote broadcasts once in `src/main/features/core/remote/index.ts:72-82`. Its comment notes that queue-state can contain hundreds of songs, but the inspected path has no item/byte ceiling. `use-remote-queue-push.tsx:17-33` projects album, artist, duration, ID, image URL, name and occurrence ID; lines 49-62 debounce queue updates and send the current occurrence separately. `use-remote-library.tsx:77-98` projects another trimmed track view. These are P11 source protocol shapes, not the normalized Page/SongSummary or the target retained QueueSnapshot. No captured full queue-state WebSocket frame or measured maximum byte size was found in the tracked evidence; the captured browser queue is persisted state.

## Decision

Add one portable codec module inside the existing storage crate. Keep Serde outside the dependency-free domain. Encode a normalized Page<SongSummary> with explicit server identity, page start, optional total count, song IDs, names and integer milliseconds. Reuse the existing retained QueueSnapshot for the other encoding, including occurrence order, priority, shuffle permutation, current occurrence and resume position. Do not introduce a competing queue DTO or state owner.

Use distinct typed schema-v1 envelopes and opaque encoded values. Caller limits cannot exceed 1 MiB and 10,000 items. Enforce byte limits before parsing and during output construction with the existing bounded writer, validate nonblank identities and retained queue invariants, and reject malformed/future data with fixed errors. Preserve unknown count versus known zero, exact u64 milliseconds and repeated songs under different occurrence IDs. Decode pages against the expected server, including empty pages. Encoding must avoid whole-value JSON clones and intermediate row vectors.

Encoded cache values retain only their validated bytes and item count. Derive query_cache::Weight from the actual UTF-8 byte length and count. A deterministic fixture must demonstrate encoding, correlated cache publication, isolated retrieval and decoding. Exact query/filter/library/page keys and request/generation checks remain the caller's responsibility; payload bytes are not an authorization token or a complete response-correlation protocol.

Debug/errors expose fixed metadata only. Schemas exclude authenticated URLs, credentials and arbitrary backend metadata; arbitrary text cannot be proven free of secrets. The decoder checks its input byte limit before typed JSON parsing, then checks the requested item cap. Temporary DTO vectors/strings and page conversion overlap are bounded by input size rather than a heap-item allocation guarantee. The encoded-byte budget does not introspect caller DTOs, decoder allocations, normalized server-ID copies, allocator overhead or transport buffers. Durable profile/import/journal/active-state values retain their separate 256 KiB budget.

## Alternatives and limits

Adding Serde to domain would couple model design to this encoding. Reusing source remote messages would imply fields and workflow ownership this P02 task does not implement. Adding a transport/cache database or a new crate would be premature. This bounded local handoff is an explicit target policy and projection, not P11 compatibility, complete normalized Song serialization, P05 queue ownership or a persisted cache.

See [contract and checks](../p02/payloads.md) and [migration records](../migration/p02-payloads.csv). Full runtime queue metadata, additional query families, actual transport reception bounds and UI/remote consumers remain open.
