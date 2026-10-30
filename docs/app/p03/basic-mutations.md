# Subsonic basic mutations and user metadata

P03.03/P03.05 adds typed favorite/rating inputs, create_favorite, delete_favorite, set_rating, start_library_scan, refresh_items and get_user_info. See [ADR-029](../adr/029-subsonic-basic-mutations.md) and [migration map](../migration/p03-basic-mutations.csv). No dependency, runtime, schema, cache invalidation or UI is added.

Favorites preserve repeated IDs in one source-compatible star/unstar request; blank/empty selections fail before dispatch. Ratings preflight all finite values/IDs/request sizes, then update IDs serially in source order without deduplication. Empty rating lists do no I/O. Both scan operations trigger the same global startScan endpoint. User metadata preserves canonical username and boolean admin role with explicit server scope; invalid role coercions are rejected. Its fixed target-username planner preserves generic credential alias protection and keeps authenticated principal separate from target data.

Receipts count validated requests. Failures retain the acknowledged prefix plus a conservative unknown-outcome flag and redacted category. A request can apply before its response is lost; no automatic retry, rollback, atomic batch or per-item success is inferred. The source setRating HTTP/protocol failure swallowing is explicitly characterized and corrected. Pending, preflight, between-request and final context checks preserve these distinctions, including late expiry after a known acknowledgement.

## Reproduce verification

Use pinned Rust/Cargo 1.92.0 with the [established launcher](../p02/README.md#reproduce-checks). Application/CI tests use tracked inputs. Optional source replay accepts hash-verified source/compiler paths:

```sh
node tools/app/p03-mutation-reference.cjs /tmp/feishin-p03-album-source /tmp/feishin-p02-settings-tooling/package/lib/typescript.js
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-domain -p feishin-storage -p feishin-subsonic -p feishin-web -p feishin-remote --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests/tooling -p 'test_*.py'
```

Five grouped Rust tests and 50 original-controller cases pass. They compare exact request arrays/order and typed acknowledgement/error policies, exercise disposable applied state with response loss, prove preflight before effects and preserve repeated IDs. They also cover request/item/response caps, safe target-username construction, protected generic aliases, canonical user projection and pending/between/final guards for successes and failures. An initial test-helper delimiter was fixed; the user-info regression then exposed reserved-username planning and passed after the fixed endpoint planner was added. After subsequent context/preflight fixes, the complete five-test target, seven request-foundation tests and 11 protocol tests pass again. Two provenance/tampering tooling tests pass. Combined verification on 2026-10-30 passed: 309 workspace tests, zero failures, 13 existing optional runtime tests ignored; 89 adapter tests. Workspace all-target and selected portable/WASM all-target Clippy, formatting, 108 tooling tests with nine optional skips (117 total), tracked evidence/links, inventory integrity, seven local reference regressions, Node syntax and diff whitespace passed. All six expression IDs and six migration rows match the immutable inventory; Cargo.lock is unchanged. One independent Sol medium reviewer found no actionable findings across correctness, compatibility, architecture, tests, edge cases, performance and operations. It independently passed five basic mutation tests, all 50 source cases, two tooling tests and diff checks. Hosted CI follows push while authorized implementation continues.

No live mutation/server/browser/runtime or phase parity is claimed. Linux/WASM only; Windows/macOS builds/tests excluded. Application invalidation/reconciliation, uncertain-outcome consumers, concrete form/runtime transport and remaining mutations/streams/reporting/jukebox remain open.
