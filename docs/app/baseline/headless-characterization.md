# P00.06 headless reference characterization

Date: 2026-10-22. This increment executes the pinned original's queue and persistence modules and its exact scrobble progress callback without a browser. It adds behavior evidence independently of screenshot collection. No Rust implementation or P01 work is present.

The [loader](../../../tools/app/p00-reference-runtime.cjs) verifies source/package/lockfile hashes against the tracked source audit before evaluating code. The actual Zustand/Immer middleware, queue actions, shuffle implementation, timestamp store, event emitter and persistence module are bundled with the frozen reference's esbuild. Boundaries are explicit: in-memory async storage/localStorage, fixture settings/radio, deterministic occurrence IDs and random bytes, and a payload-free logger. Engine event consumers and physical storage are absent. This is broader than the earlier isolated helpers, but still does not mount React, open Electron or exercise real IndexedDB.

| Evidence | Observations |
| --- | --- |
| [25 queue/persistence cases and 1,000-command trace](../../../tests/fixtures/p00/state-traces.json) | Every Play enum variant is invoked with a selected target; INDEX insertion has no effect in this direct store path. Duplicate song IDs retain distinct occurrences. Tested shuffle toggles preserve the current occurrence. Seeded add/remove/move/shuffle/next/repeat operations produced no checked canonical-resolution, permutation or priority-resolution violations. |
| Same state trace | Manual Next versus natural end across repeat modes, Previous at 10/11 seconds, pinned Up Next boundary and pause-at-end one-shot behavior have snapshots/events. At 11 seconds Previous emits seek-to-zero while the fake timestamp remains 11: no engine consumes that command in this harness. Do not mistake emitted command for completed seek. |
| Persistence cases | Embedded v3 queue migration prunes orphan song-map entries while retaining duplicate occurrences. Column arrays replace defaults. Corrupt main state yields null; corrupt split queue is omitted. Failure on the second migration write is surfaced; retry recovers with the already-written split queue. This demonstrates selected synthetic interruption points, not transactional durability. |
| Settings case | Full `splitSettingsStorage` execution confirms the category-write defect and additionally shows that legacy combined-state migration omits `autoDJ`. After migration, a normal category update leaves old category data intact while its stored version advances to 34. These are synthetic historical-shape inputs, not captured user exports. |
| [8 scrobble progress cases](../../../tests/fixtures/p00/scrobble-traces.json) | At 0.5x/1x/2x sampled media advancement, the 60-second media-time threshold is reached after 120/60/30 synthetic wall-second samples. Pause samples are not credited; large jumps rebaseline, restart clears accumulation, private mode emits no requests, and seek-to-start permits a later submission. |
| Same callback trace | A 180000 ms normalized duration becomes 180000000 in Subsonic submission arguments and 1800000000000 in Jellyfin submission arguments; a normal 10-second Jellyfin progress sample correctly produces 100000000 ticks. Submission is marked before mutation completion. No request success callback is invoked here, and no downstream adapter/HTTP consequence is claimed. |

The progress harness extracts exact declarations with TypeScript 6.0.3 and checks the source hash first. It supplies refs, a player snapshot, settings, `useCallback` identity and a request recorder. Its samples are controlled inputs, not measured playback clocks. Song-change debounce, React re-render/unmount, notification timers, actual network failure/retry and audio device speed remain separate cases.

The later [adapter replay](adapter-characterization.md) feeds saved callback requests through the original controllers/clients to an offline Axios boundary. It confirms that Jellyfin retains the oversized submission ticks in its request body and returns before a held acknowledgement; Subsonic/Navidrome omit submission position and wait. This extends payload evidence without establishing live server/plugin or engine consequences.

One additional ambiguity is now recorded: after last-item stop with repeat none, the store resets index to zero before emitting `PLAYER_STOP`, so the captured stop event names the first occurrence. Engine/reporting consequences and desired compatibility remain unverified. Preserve the trace and investigate consumers before choosing a difference; the immutable original was not patched.

Run from the repository root with the optional frozen reference environment:

```sh
node tests/reference/p00-state-traces.cjs /tmp/feishin-p00-reference /tmp/feishin-p00-reference/node_modules/.pnpm/node_modules/esbuild/lib/main.js tests/fixtures/p00/state-traces.json
node tests/reference/p00-scrobble-traces.cjs /tmp/feishin-p00-reference /tmp/feishin-p00-tooling/node_modules/typescript/lib/typescript.js tests/fixtures/p00/scrobble-traces.json
```

Both commands assert exact equality against the saved evidence. The traces, scenario code and deterministic seeds are tracked; the original source and installed dependencies remain optional local reference inputs. [Runtime migration anchors](../migration/p00-runtime-map.csv) remain `unmapped`; service captures and headless source execution are identified separately. P00.06 still needs song-change/Auto DJ races, complete export/import UI and engine/reporting integration evidence. Screenshot capture alone will not close those cases.
