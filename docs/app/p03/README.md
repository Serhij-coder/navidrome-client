# P03 Subsonic foundation and sequencing exception

The maintainer explicitly authorized a bounded P03.01 implementation on 2026-10-29 while P02 remains open. This exception permits base URL/subpath handling, query encoding, salted/explicit legacy authentication parameters, client/version defaults and credential redaction. It does not declare P02 or P03 complete, authorize later adapter operations or waive existing source, fixture, runtime, packaging or phase-exit evidence requirements. P00/P01 gates also remain open. [ADR-012](../adr/012-subsonic-request-foundation.md) records the adapter boundary and deliberate source differences.

This branch starts directly at `1a99711`, independently of `p02-credential-executor`. That branch was preserved after [its CI](https://github.com/Serhij-coder/navidrome-client/actions/runs/40873448430) passed all five Linux/WASM/AppImage jobs at `9ab58b5`. The primary orchestrator owns P02.07 and integration; this branch neither merges nor depends on the credential executor. `STATUS.md` is intentionally reserved for the primary orchestrator's combined status.

## Implemented library contract

The small [feishin-subsonic crate](../../../crates/subsonic/src/lib.rs) constructs wire URLs without HTTP I/O. It adds no library reads, server mutations, login handshake, playback, UI, automatic credential remembering or storage schema. Domain/cache/query code and existing browser probes remain unchanged.

- `ServerBase::parse` validates/canonicalizes an HTTP(S) base, preserves subpaths, accepts intentional LAN/loopback/IPv6 hosts and rejects URL credentials/query/fragment, controls, backslashes and malformed percent escapes. Surrounding spaces are trimmed. Exactly one trailing slash is removed before `/rest` is appended. An already `/rest` base is not inferred or rewritten, so it becomes `/rest/rest`; repeated trailing slashes are preserved according to the observed source rules.
- `Auth::salted` consumes a trusted caller-supplied `Entropy`, generates 12 alphanumeric characters with bounded unbiased sampling, and computes lowercase MD5 of the exact UTF-8 password bytes followed by salt bytes. There is no Unicode normalization. Salt/token are reused when the same Auth constructs another request. The portable boundary accepts non-Send providers. It has no built-in platform RNG, fixed fallback salt or entropy-failure fallback.
- `Auth::legacy` explicitly opts into a plaintext password query parameter. Salted construction never falls back to legacy. Empty strings are retained at this wire layer as in the original controller's string checks; server acceptance and UI policy are not inferred.
- `ServerBase::request` takes a single validated endpoint segment and ordered decoded string pairs. It appends `c=Feishin&f=json&v=1.13.0`, username and either `s/t` or `p`, then RFC3986-encodes keys and values exactly once. Spaces become `%20`, literal plus becomes `%2B`, repeated keys repeat, and empty values stay empty. Caller pairs cannot override default/authentication names or their reserved aliases.
- Authentication material, bases and `RequestUrl` have constant redacted Debug output, no Display/serialization implementation, and no raw data in error values. Only `RequestUrl::expose_for_transport` exposes the complete URL. Keep that borrowed string inside the eventual transport boundary; it is not safe diagnostic data. No arguments/environment variables/logs receive real credentials in this task.

Each base, username, password, endpoint, query key and value is limited to 8192 UTF-8 bytes; there are at most 128 query pairs and a successful URL is at most 65536 bytes. Endpoint errors are `InvalidEndpoint`; other fixed categories are `InvalidBase`, `InvalidQuery`, `InputTooLarge` and `EntropyUnavailable`. These are constructor bounds, not limits on the caller's allocations, a transport body or remote server response. Entropy stops after at most 256 one-byte draws; the trusted provider must fill each byte and avoid secret-bearing diagnostics.

## Source fidelity and intentional differences

The [migration map](../migration/p03-foundation.csv) binds stable upstream IDs to target symbols, dispositions, tests and remaining gaps. [Synthetic source-replay fixtures](../../../tests/fixtures/p03/subsonic-foundation.json) include source/function hashes, frozen dependency hashes, exact UTF-8 salted tokens and seven matching constructed URLs. The eighth characterizes a URIError when the source client URI-decodes a raw username containing `%`. The target retains raw usernames and encodes once; both literal `%` and `%2F` have regressions. This deliberate correction is recorded in ADR-012.

The typed pair API replaces the source TS route-string parser. It preserves literal `null`/`undefined` and bracketed names as explicit string data instead of stripping/dropping them. No route-string compatibility parser or endpoint DTO is claimed. Reserved parameter rejection and stricter external input validation are deliberate boundary choices. The source's response envelopes, error codes, permissions, feature-gated form POST and JSON POST paths are deferred to P03.02 and later work. Local/remote endpoint selection remains a caller decision; no new server/profile owner is introduced.

## Build, tests and reference replay

Use Rust/Cargo 1.92.0 and the [established launcher](../p02/README.md#reproduce-checks). Tests are in the top-level [Subsonic test target](../../../tests/subsonic/foundation.rs), explicitly wired in Cargo. Default builds/tests need only tracked application inputs, without ignored plans, source archives, source compilers or reference JavaScript packages.

```sh
cargo test -p feishin-subsonic --locked
cargo clippy -p feishin-subsonic --all-targets --locked -- -D warnings
cargo clippy -p feishin-subsonic --all-targets --target wasm32-unknown-unknown --locked -- -D warnings
python3 -m unittest discover -s tests/tooling -p 'test_p03_subsonic.py'
```

The optional [replay tool](../../../tools/app/p03-subsonic-reference.cjs) first verifies four original file hashes, seven selected expression hashes, the TypeScript 6.0.3 compiler hash and five dependency entry/bundle hashes. It evaluates only those expressions with deterministic Math.random and stubbed responses/Axios; it does not initialize original modules, execute package lifecycle hooks, access a profile, contact a server or perform an actual login. It uses upstream qs 6.15.2, md5 2.3.0, charenc/crypt 0.0.2 and is-buffer 1.1.6. Their tarball SHA-512 integrities were matched against the pinned pnpm lock before extraction. Dependency layout is `DEPENDENCIES_ROOT/qs/dist/qs.js`, `md5/md5.js`, `charenc/charenc.js`, `crypt/crypt.js`, `is-buffer/index.js`; package installers are unnecessary. The fixture records the exact entry hashes.

```sh
node tools/app/p03-subsonic-reference.cjs SOURCE_ROOT TYPESCRIPT_JS DEPENDENCIES_ROOT
```

`SOURCE_ROOT` must contain the original `src/...` paths from the pinned archive; `TYPESCRIPT_JS` is the trusted hash-matched compiler module. Default mode compares without changing the fixture. `--write` explicitly regenerates synthetic expected records and must be followed by review/tests. Optional full tooling replay uses `P03_SOURCE_ROOT`, `P03_TYPESCRIPT` and `P03_DEPENDENCIES` pointing to these trusted inputs. The default tooling test still checks provenance against the tracked source audit and rejects modified source before compiler loading. This is controlled reference execution, not a general untrusted-JavaScript security sandbox.

See [verification and review](verification.md) for exact commands/results, dependency/notice evidence and remaining gaps. CI's existing workspace jobs discover the new crate/tests; the existing portable WASM Clippy command explicitly includes this adapter. No general CI behavior or platform scope is changed. Windows/macOS builds and tests are excluded; Linux x86_64/aarch64 AppImage smoke jobs remain required and retain their prior scope.

## Integration handoff and open work

Integrate this branch separately after review; no executor/main merge or cherry-pick was performed here. Root Cargo workspace/lock entries and the adapter-specific WASM CI package list are the main adjacent-hunk integration points. The existing package versions/checksums are retained; only md-5 0.10.6 and the new local crate are added. [Third-party notices](../../../THIRD_PARTY_NOTICES.md) cover source attribution, the protocol digest dependency and external reference infrastructure.

Future composition must provide secure native/browser entropy, an HTTP transport that keeps authenticated URLs and backend wire errors out of diagnostics, cancellation/deadlines, response/envelope handling and target policy for TLS/redirects. Real Subsonic authentication and generic/extension-enabled server conformance remain unverified. There is no SecretStore/credential-executor integration, browser runtime evidence or secure-memory-erasure guarantee. P03.02-P03.07 and all P02/P03 exit requirements remain open.

The maintainer subsequently authorized a managed parallel [P03.02 protocol continuation](protocol.md) while P02 remains open. It supplies bounded envelope/error handling, conditional extension discovery, server-bound form planning and cooperative local transport guards. See [verification](protocol-verification.md); no production HTTP/runtime or phase exit is implied.

The [P03.03 song detail continuation](song-reads.md) adds one normalized `getSong` operation with exact source fixtures and typed context checks. [Verification](song-reads-verification.md) separates this tested local adapter from remaining library reads, live conformance and transport integration.

The [album/album-artist continuation](album-reads.md) adds two guarded detail operations with nested song/disc normalization and an aggregate work budget. [Verification](album-reads-verification.md) and [ADR-016](../adr/016-subsonic-album-reads.md) record source comparisons and deliberate differences.

The [playlist snapshot continuation](playlist-reads.md) adds separate metadata, ordered entry and song-ID reads, preserving duplicate songs with scoped position identities. [Verification](playlist-reads-verification.md) records original-controller replay and bounded local transport tests.
