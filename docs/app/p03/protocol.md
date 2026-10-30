# P03.02 protocol foundation

The maintainer authorized continued bounded P03.02 implementation on 2026-10-29 while P02 remains open, with a managed second orchestrator working in an isolated worktree. [ADR-013](../adr/013-subsonic-protocol.md) records the protocol decisions and deliberate source differences. This continuation adds a portable library contract, not a login screen, production HTTP transport or a P02/P03 exit. [P03.01](README.md) authentication, reserved-name and encoded-once rules continue unchanged.

## Request planning

[feishin-subsonic](../../../crates/subsonic/src/protocol.rs) exports `plan_request`, `PlannedRequest`, `PostPreference`, `Method`, `ProtocolTransport`, `TransportResponse`, `discover`, `Discovery` and closed exchange/HTTP/protocol/transport error types. Wire JSON stays inside this adapter; no normalized library operation or UI DTO is added to domain.

`PostPreference::Get` always keeps the validated encoded query on the URL. `PreferForm` uses form POST only when the supplied discovery result advertises `formPost` version 1 for the same canonical base; otherwise it uses GET. `RequireForm` returns `ExchangeError::Unsupported` without that support. A result obtained from one server base cannot authorize form POST for another base. Equivalent canonical spellings of the same base remain usable. The caller owns refreshing discovery after server/profile changes.

Form POST moves every default/auth/query parameter into the body, including repeated keys and empty values, and reports `application/x-www-form-urlencoded` through `content_type`. It uses the P03.01 RFC3986 encoder: spaces are `%20`, literal plus is `%2B`, and input is encoded once. The URL has no query for this form mode. No JSON POST body, `getTranscodeDecision.view` special handling or general mutation endpoint is implemented. Planning a validated endpoint does not establish support for its operation.

`PlannedRequest` has constant redacted Debug. Complete authenticated URLs and bodies are exposed only by `expose_url_for_transport` and `expose_body_for_transport`; use these only inside a trusted transport implementation. The existing `RequestUrl::expose_for_transport` boundary is unchanged. Do not serialize, log or hand authenticated values to diagnostics. Memory zeroization, secure platform entropy and credential-consent composition remain open.

## Discovery and errors

`discover` sends authenticated GET `ping.view`, then GET `getOpenSubsonicExtensions.view` only when the ping advertises `openSubsonic: true` and a nonempty string `serverVersion`. Missing optional ping fields end discovery without extension requests. Missing extension arrays yield no extensions; invalid types/entries fail closed. Results retain the protocol and optional server version separately.

Known extension names are `formPost`, `indexBasedQueue`, `playbackReport`, `songLyrics`, `topSongsByArtistId`, `transcodeOffset` and `transcoding`. Unknown names are ignored after validating their entry shape. Versions must be positive integers. Each extension list and each version list obeys the context item limit; duplicate names use the last version list. Empty version lists grant no support. `Discovery::supports(name, version)` uses explicit membership, and `versions` returns only that name's retained advertised list. No server-family inference, jukebox probe, account permissions, or application capability-map composition is added. Discovery Debug is constant and redacted, including arbitrary server-supplied version strings.

A successful envelope is an object under `subsonic-response`, with nonempty string `version` and `status: "ok"`. A failed envelope has `status: "failed"` and an integer numeric or integer-string code. Malformed JSON or required/typed fields return `Malformed`. Authentication code 40 has its own category; standard codes 0/10/20/30/50/60/70 map to closed categories and other integer codes map to `Other`. Server error messages and raw JSON parser errors are discarded. A failed code 0 remains a protocol failure; there is no silent-error-to-success conversion.

Only HTTP 200 establishes a successful JSON exchange, matching the source discovery controller. Other statuses remain separate closed HTTP categories; they are never substituted with protocol codes. Network/timeout/cancellation/provider failures cross the local transport boundary only as the closed `TransportError` enum. Errors contain no raw server body, URL, credential or provider error chain.

## Operation bounds and transport responsibilities

`discover` consumes the existing domain `OperationContext`, `Clock` and local future boundary. Before dispatch it checks cancellation, monotonic deadline and combined URL/body request bytes. It checks cancellation/deadline on every poll and after completion, requires the response's full request/generation/deadline correlation to equal the issued context, checks response bytes before JSON parsing, and checks completion context after parsing. Cancellation precedes deadline; equality with the deadline and configured limits is accepted, while exceeding a limit fails.

The boundary supports non-Send browser-style futures. A concrete transport must bound allocations while receiving data and arrange wakeups/cancellation/deadlines while pending. This wrapper can reject stale/oversize/late completion and drop a polled pending future after cancellation/deadline, but it cannot independently wake an uncooperative future, interrupt synchronous transport work or prove a remote request was undone. The response-byte check here occurs after the transport has allocated its `Vec`; the transport must enforce the same limit during reception. No actual network implementation, socket cancellation or timer/executor is selected.

## Checks and remaining scope

Tests live in the top-level [protocol Cargo target](../../../tests/subsonic/protocol.rs), wired by the [crate manifest](../../../crates/subsonic/Cargo.toml). Fixtures are synthetic inline JSON with dummy credentials, not captured live server responses. They exercise envelope/discovery/form behavior and deterministic local transport guards without a server or ignored reference package.

Use Rust/Cargo 1.92.0 and the [established launcher](../p02/README.md#reproduce-checks):

```sh
cargo test -p feishin-subsonic --locked --offline
cargo clippy -p feishin-subsonic --all-targets --locked --offline -- -D warnings
cargo clippy -p feishin-subsonic --all-targets --target wasm32-unknown-unknown --locked --offline -- -D warnings
cargo fmt --all -- --check
python3 tools/app/check-p00.py
```

See [verification](protocol-verification.md) and the [migration map](../migration/p03-protocol.csv) for exact checks, source anchors and limitations. Generic/extension-enabled live server conformance, authentication/re-login, production transport policy, browser CORS/runtime, streaming reception limits, full capability composition and P03.03-P03.07 remain open. No audio, device, packaged-adapter or full P03.02/P03 completion is established by deterministic doubles.

The [album-count continuation](album-counts.md) adds optional selected total_count_header metadata to TransportResponse, capped at 128 UTF-8 bytes and included with body bytes in the response budget. Existing operations ignore the value after those guards; only the general album-count path interprets it after HTTP/envelope success. Debug stays fully redacted. This selected-value budget does not replace a production transport's full-header or streaming reception limits.
