# P01.05 Windows mpv IPC candidate

This increment extends the local-file mpv feasibility boundary to Windows. It is not a production engine, OS media-control integration or verified Windows audio result. The existing Unix process/JSON behavior remains shared. Windows runtime tests have been added to the normal workspace/hosted native suite; the [58814c1 hosted run](evidence/hosted-windows-ipc.json) passed the Windows native job containing all four new tests, plus the other six jobs.

The baseline `src/main/features/core/player/index.ts:31` selects a Windows named pipe, and the stable migration anchors `:152:19` / `:267:14` cover creation/teardown. The candidate uses an unpredictable session name instead of the baseline's process-only name. The pipe server PID must match the owned child before any JSON command is sent. Local files, current/next append, command IDs, bounded reply frames, stale-reply rejection, generation invalidation and managed-child teardown retain the existing prototype contract. Non-Unicode Windows paths return a controlled error before filesystem access.

The transport uses target-only Tokio 1.48.0 and interprocess 2.4.6 through safe APIs. The workspace keeps `unsafe_code = "forbid"`. A current-thread runtime belongs to the connector/stream; no runtime is installed globally or in the UI. Tokio ClientOptions opens the local pipe with explicit identification-only security quality of service. Missing/busy pipes return immediately; no blocking connection worker is used. A duplicated handle is inspected through interprocess's synchronous safe wrapper solely to query the server PID, then closed before protocol I/O. That metadata wrapper is marked clean immediately because it sends no bytes. The existing startup loop checks the child and its three-second connection deadline; the initial protocol request has its own three-second deadline. Each read/write uses the remaining absolute request time, including partial replies. Errors invalidate the generation.

The stream has no userspace write buffer. Request replies acknowledge operations; quit is best effort before bounded graceful/forced child disposal. The actual I/O stream is Tokio NamedPipeClient, with direct handle disposal and no interprocess writer/flush pool. Unread final quit/error bytes may be discarded; the process deadline owns final shutdown. Review found that interprocess 2.4.6's Tokio OwnedHandle conversion can return an already-consumed handle on registration failure. That conversion and the library's Tokio feature are excluded; the synchronous metadata wrapper does not transfer its handle to Tokio. All APIs remain blocking and must stay outside UI/event executors. This does not establish a production Tokio application runtime or a second playback owner.

The explicitly wired tests use a tracked Rust pipe peer, not Python scripts or a real music server. Per-test hard-link aliases select normal, malformed, oversized, process-death, missing-pipe, slow-partial-reply and stalled-quit behavior without writing a new executable. Their directory is on the fixture's volume and requires no Windows symbolic-link permission. Three integration tests cover correlations/events, queue count, invalid Unicode/seek inputs, generation lifetime, frame limits, deadlines and shutdown/drop. A private transport test rejects a server-PID mismatch before protocol I/O. No external audio device or host capture is opened.

On Windows, run the normal required tests or this focused target:

```powershell
cargo test --workspace --locked
cargo test -p feishin-native-spike --test windows_pipe --locked
```

An independently installed trusted mpv is still required for a real-engine probe. The following PowerShell commands generate a new owned tone (creation refuses overwrite) and discard audio through mpv's null output:

```powershell
cargo run -p feishin-desktop -- --tone "$env:TEMP\feishin-p01-new-owned.wav"
cargo run -p feishin-desktop -- --mpv-probe-null C:\Tools\mpv\mpv.exe "$env:TEMP\feishin-p01-new-owned.wav"
```

Verification: 32 default Linux Rust tests passed, with the five external media and two performance tests explicitly ignored. The five external probes passed separately in the owned Ubuntu 24.04 x86_64 container after the common transport extraction. Windows x64 MSVC native/desktop all-target Clippy and x64 GNU/arm64 MSVC native all-target checks passed, including the Windows test/peer sources. Default/wgpu/WASM Clippy, both macOS desktop compile checks, release web bundles and collection of all 371 registry notice sets passed. Those compile results did not execute the four Windows-only tests; the subsequent hosted native job did. All 49 Python tooling tests passed with the frozen reference inputs; inventory, seven local reference regressions, node syntax and documentation/whitespace checks also passed. No new AppImage was built locally for this target-only addition; earlier artifacts remain historical.

Actual mpv compatibility (including canonicalized Unicode paths), playback/device/media keys, ACL/session behavior, handle/resource measurements, permissions/capture and physical target certification remain open. Both independent reviewers accepted the final implementation. The correctness review found the dependency conversion defect described above; the I/O/metadata ownership split removes that path, affected target checks passed again, and both reviewers accepted the focused fix. The correctness reviewer independently reran Windows x64 MSVC all-target Clippy; the validation reviewer independently reran Unix native/supervisor tests and checked final ownership/whitespace. The subsequent hosted results above cover fixtures, with real mpv/device certification still open. Full P01 exit is not claimed.
