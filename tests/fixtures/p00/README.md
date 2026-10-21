# P00 synthetic fixtures

These are hand-authored synthetic inputs, never captured user/server data. They contain no credentials or authenticated URLs. [songs.json](songs.json) has six positive song DTOs and six malformed variants for Jellyfin, Navidrome native and Subsonic. They deliberately share song IDs across servers and include missing artwork, optional metadata and Unicode. Duration is 180 seconds on Navidrome/Subsonic and 1,800,000,000 ticks on Jellyfin, normalizing to 180,000 ms.

The [optional reference harness](../../../tools/app/p00-wire-check.cjs) runs those DTOs through pinned upstream schemas/normalizers. See [results](../../../docs/app/baseline/wire-results.json). Jellyfin emits two warnings for missing media sources; the harness records the count and suppresses raw payload logging. Passing these assertions proves only IDs/server IDs/duration for these inputs, not complete model equality, HTTP/auth/protocol behavior or live-server conformance.

[server-scenarios.json](server-scenarios.json) specifies 12 old/current/non-admin/missing-extension profiles and failure/identity cases. Its version strings are synthetic boundary inputs, not compatibility claims. HTTP cases specify expected treatment but are not executed adapter tests; actual server captures, permission responses, extension discovery and non-admin mutation outcomes remain pending P00.04. Navidrome native/Subsonic delegation still needs both transports characterized. Generic Subsonic and extension-enabled OpenSubsonic need separate live runs.

Generate deterministic, bounded wire pages without a database or ignored references:

```sh
python3 tools/app/p00-library.py jellyfin --count 100000 --offset 0 --limit 500
python3 tools/app/p00-library.py navidrome --count 10000 --offset 9999 --limit 500
python3 tools/app/p00-library.py subsonic --count 100000 --offset 100000 --limit 500
```

Run commands from the repository root. Count is bounded to 100k, page size to 500; empty/last pages and identity are tested. Jellyfin includes total count in its envelope; Navidrome emits the native JSON array (total count belongs in response headers); Subsonic emits the search3 song envelope without inventing a total. These pages are recipes, not a running fixture server. They contain synthetic file paths but no audio bytes; use owned test tones on disposable servers for playback/transition tests. The schema harness checks the tracked samples, not all generated pages or complete server versions.

P00.06 helper cases and expected outcomes are preserved in [characterization](../../../docs/app/baseline/characterization.md) and [source audit](../../../docs/app/baseline/source-audit.json). Historical serialized settings/queue exports must still be captured from an isolated original profile; generated target-shaped data cannot stand in for historical evidence.
