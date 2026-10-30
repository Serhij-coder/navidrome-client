# ADR-028: root indexes and recursive Subsonic folders

Status: accepted for bounded P03.03 continuation, 2026-10-30. No UI, filesystem traversal, playback or phase exit.

## Evidence and decision

Pinned Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8` controller getFolder at `1037:16` (1037-1137), normalizeFolder at `457:25` (457-495), getLibraryId at `2504:1`, sortSongList at `144:29` and the existing song normalizer define this operation. The original-expression replay binds 13 expressions, five enum bindings and the getIndexes/getMusicDirectory API routes. Use existing portable FolderQuery/Folder/FolderChildren and add one adapter get_folder; no domain expansion, crate, dependency, schema or concrete transport.

Exact query ID "0" selects synthetic root indexes. Forward only the first folder ID there, flatten index.artist in source order, project ID/name before optional name search, then normalize selected folders and stably sort lowercase UTF-16 names in the requested direction. Root indexes.child and shortcut are ignored, as in source. Return name "~", no parent, absent image fields, empty songs and scoped child folders. Child folders have explicit null image fields when cover art is absent. The source always sorts folders by name, regardless of song sort selection; root never invokes song RNG.

Other IDs call getMusicDirectory using only the requested ID. Recursively normalize child nodes before filtering. Only isDir exactly true chooses a folder; other values go through song normalization. Preserve nested folder/song order, metadata, repeated IDs and server scope. Do not infer sorting/filtering of descendants, album disc-title maps or occurrence IDs. A private normalization node retains raw float-duration sort keys until the requested directory's immediate songs are sorted, without cloning normalized song collections.

Search filters immediate folders by name and immediate songs by title, album or artistName. It does not search albumArtistName or descendants. Folder sorting always uses stable lowercase names; song sorting uses the shared ADR-026 contract/RNG only on immediate remaining songs. Nested children retain original normalized order. Empty search has no filter effect; non-root music-folder input has no wire effect.

## Checked differences and bounds

Retain explicit scoped identities and checked field/ID/time policies from ADR-014/ADR-016. Reject a returned requested-directory identity mismatch as Stale, continuing the detail-read policy; the fixture preserves the original mismatched result and explicit target error. Parent IDs require the source schema's optional string type; numeric/object values fail instead of becoming incompatible domain IDs. Missing/null optional parents collapse to None. Root image-field absence stays distinct from normal directory image nulls. Malformed optional parents/arrays or required fields return typed failures. Source HTTP error text containing a serialized body remains replaced by redacted typed HTTP categories under ADR-013.

Validate server/query ID, search length and all supplied folder selector cardinalities before dispatch. One aggregate budget counts root/index/artist rows or complete recursive directory/song nodes plus supplied song metadata. Rows later excluded are still charged. Normalization receives acyclic JSON values already subject to decoder nesting and byte limits; no arbitrary recursive data source or OS traversal is introduced. Work limits are logical bounds, not exact allocation bounds. Cached UTF-16 folder keys avoid recomputing lowercase names per comparison and stable descending keys preserve equal-name ties. Pending/final cancellation/deadline checks and exact correlation remain local/non-Send; malformed nested data or errors publish no partial tree.

The shared created-date sort deliberately uses the UTC ISO policy from ADR-026; host-local/general Date.parse compatibility is not expanded. The caller binds profile/base/auth and schedules bounded CPU work outside UI painting. No browser or packaged adapter runtime parity is inferred from compilation or fixtures.

See [contract and verification](../p03/folders.md) and [migration map](../migration/p03-folders.csv). Remaining backend metadata/read operations, mutations/streams/reporting, concrete transport, live conformance and all phase exits remain open.
