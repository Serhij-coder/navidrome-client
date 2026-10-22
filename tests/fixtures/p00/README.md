# P00 synthetic fixtures

This directory distinguishes synthetic inputs, headless original-code traces and redacted disposable-server captures. No real user profile, credentials or authenticated URLs are included. [songs.json](songs.json) is hand-authored synthetic data: six positive song DTOs and six malformed variants for Jellyfin, Navidrome native and Subsonic. They deliberately share song IDs across servers and include missing artwork, optional metadata and Unicode. Duration is 180 seconds on Navidrome/Subsonic and 1,800,000,000 ticks on Jellyfin, normalizing to 180,000 ms.

The [optional reference harness](../../../tools/app/p00-wire-check.cjs) runs those DTOs through pinned upstream schemas/normalizers. See [results](../../../docs/app/baseline/wire-results.json). Jellyfin emits two warnings for missing media sources; the harness records the count and suppresses raw payload logging. Passing these assertions proves only IDs/server IDs/duration for these inputs, not complete model equality, HTTP/auth/protocol behavior or live-server conformance.

[server-scenarios.json](server-scenarios.json) specifies 12 synthetic old/current/non-admin/missing-extension profiles and failure/identity cases. Its version strings are boundary inputs, not compatibility claims. Those HTTP cases remain proposed expectations, separate from [live-responses.json](live-responses.json): 92 actual redacted responses from six disposable old/current Jellyfin/Navidrome/Gonic instances, both roles, missing extensions and playlist mutations. See the [runbook](../../../docs/app/baseline/live-servers.md) for image digests, provisioning and limitations. Direct service API capture is not pinned Feishin adapter execution or complete backend conformance.

Generate deterministic, bounded wire pages without a database or ignored references:

```sh
python3 tools/app/p00-library.py jellyfin --count 100000 --offset 0 --limit 500
python3 tools/app/p00-library.py navidrome --count 10000 --offset 9999 --limit 500
python3 tools/app/p00-library.py subsonic --count 100000 --offset 100000 --limit 500
```

Run commands from the repository root. Count is bounded to 100k, page size to 500; empty/last pages and identity are tested. Jellyfin includes total count in its envelope; Navidrome emits the native JSON array (total count belongs in response headers); Subsonic emits the search3 song envelope without inventing a total. These pages are recipes, not a running fixture server. They contain synthetic file paths but no audio bytes; use owned test tones on disposable servers for playback/transition tests. The schema harness checks the tracked samples, not all generated pages or complete server versions.

P00.06 helper cases and expected outcomes are preserved in [characterization](../../../docs/app/baseline/characterization.md) and [source audit](../../../docs/app/baseline/source-audit.json). Historical serialized settings/queue exports must still be captured from an isolated original profile; generated target-shaped data cannot stand in for historical evidence.

[state-traces.json](state-traces.json) records 25 source queue/persistence cases and a seeded 1,000-command trace. [scrobble-traces.json](scrobble-traces.json) records 8 exact progress-callback cases. Their [scenario scripts](../../reference/p00-state-traces.cjs) and [callback script](../../reference/p00-scrobble-traces.cjs) assert exact reproduction when the optional pinned source/dependencies are available. The [headless guide](../../../docs/app/baseline/headless-characterization.md) states all fake boundaries and remaining React/audio/network gaps. Synthetic historical-shape storage inputs here are not actual exported profiles.

[adapter-traces.json](adapter-traces.json) records 54 original controller/client/interceptor/normalizer cases. Inputs identify saved live-response indexes or synthetic transport responses; capture/callback hashes bind their source fixtures. [Scenarios](../../reference/p00-adapter-traces.cjs) and the [adapter guide](../../../docs/app/baseline/adapter-characterization.md) describe offline replay, including omitted capture headers, diagnostic schema warnings, submission units/completion, positional playlist IDs and a bounded Jellyfin queue payload. No new live HTTP/authentication, remote mutation or server/plugin interpretation is claimed.
