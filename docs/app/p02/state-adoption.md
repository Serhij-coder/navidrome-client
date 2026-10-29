# Durable state adoption boundary

P02.05 now provides `state::StateRepository` in the existing storage crate. It owns the active value, the staged import journal and the earlier profile reader through one mutable worker API. This is the durable repository boundary for a future application owner. The P01 shells still do not open these stores, hydrate UI settings, restore playback or execute production worker messages.

The subsequent [storage session/native worker](state-worker.md) now supplies typed review/apply dispatch and a bounded native executor around this repository. Shell composition and background browser execution remain separate.

## Reviewed replacement and recovery

`load` is read-only. Before an active value exists it synthesizes revision zero from the existing schema-v1 non-secret profile, including typed partial-recovery diagnostics. Missing profiles use existing defaults; malformed/future whole profiles fail without writes. Other fields start at `MigrationData` defaults. Neither the legacy profile nor the staged import becomes active implicitly.

`prepare_adoption` reads the strictly validated stage and returns immutable before/after snapshots. The entire staged partial projection replaces the active retained schema: an import prepared against an empty stage can remove existing active profiles or preferences. Callers must review this replacement explicitly. It does not merge the staged projection with active state or promise complete historical settings hydration. An empty stage is rejected. `prepare_save` supports validated local changes by the future state owner. Both operations check the candidate plus backup size before returning a reviewable plan.

`apply` checks exact active bytes, exact stage bytes for adoption and exact legacy bytes while active state is absent. Intervening edits reject the prepared operation; a source that becomes invalid also fails closed. A fresh equivalent operation is a no-op after activation. The first save persists even identical defaults, establishing the active namespace as authoritative. After activation, the old profile is ignored, including if subsequently corrupted. No API in this boundary writes the old profile or source stage during adoption/save/rollback.

One schema-v1 active envelope contains the current snapshot, one backup and a checked monotone revision. The first backup is the synthesized legacy snapshot, so undo restores the state shown before adoption. `prepare_rollback` reviews the exact backup; applying it increments the revision and keeps the replaced state as the next backup, allowing one-level undo/redo. Future schemas, malformed or invalid snapshots, inconsistent revision ordering, exhausted revisions and oversized envelopes are protected from mutation. All current and backup values pass the existing non-secret target validator.

Only the active value is replaced. Native storage uses `feishin-rust-dev-p02/state-v1.json` below the explicit development root. Browser storage uses `feishin-rust:dev:p02:state:v1`. The earlier `profiles-v1.json`/profile key and `imports-v1.json`/stage key remain independent, unchanged inputs. Keeping all active retained fields and their backup in one value avoids claiming atomicity across separate writes.

The repository returns a new snapshot only after a successful replacement. An I/O error after visible replacement remains an error: reload/reconcile before any further action. There is no cached published state or automatic retry. Atomic replacement has the existing filesystem/localStorage limits; it is not cross-process/tab compare-and-swap, a writer lock or power-loss certification. The application must serialize calls through one owner. Native parsing and I/O need a bounded worker; browser futures remain free of a universal `Send` requirement. Production scheduling is still open.

## Evidence and remaining work

The pinned source `7d6ad318c79802e226996cfc39acba6b05bb22e8` persists auth/settings/player separately: auth merge at `auth.store.ts:114`, settings migrate at `settings.store.ts:2442`, and player then queue writes at `utils.ts:119-143`. Those are exact stable inventory anchors in the [adoption migration map](../migration/p02-state.csv). A unified non-secret active envelope and explicit replacement review are target policy, not a claim that the original split writes were transactional. [ADR-004](../adr/004-persistence.md) records the decision and alternatives.

The [state integration target](../../../tests/storage/state.rs) is explicitly wired into Cargo. The [browser harness](../../../tests/browser/p02-state.cjs) uses the same test-only WASM example as the profile/import tests. See [verification](state-adoption-verification.md) for actual commands/results. Fixtures retain repeated song occurrences and exclude imported credentials. No live Feishin profile is opened.

Full historical fields and real older-user exports remain P02.05 work. Application worker/shell composition remains P02.05/P08; production queue ownership and playback restoration remain P05/P06. P02.06 credential policy/platform stores and P02.07 cache/cancellation remain explicitly open. P00/P01/P02 exits and release/parity claims remain unestablished. Current build verification is Linux and WASM/browser only, per the maintainer's instruction; Windows/macOS code and historical evidence are retained without new runs.
