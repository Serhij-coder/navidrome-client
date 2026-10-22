# P00.04 live disposable server fixtures

Date: 2026-10-22, Linux amd64, rootless Podman. [92 redacted HTTP responses](../../../tests/fixtures/p00/live-responses.json) cover these actual servers. The capture is direct service API evidence; it does not execute Feishin's adapters, authenticate a browser or prove audio playback. All six servers were stopped after capture. Their temporary profiles contain generated credentials and must stay outside Git.

A subsequent [offline adapter replay](adapter-characterization.md) now executes selected original controller/client/interceptor/normalizer paths against these saved bodies/statuses. Its synthetic boundaries and request differences are explicit; it does not turn the direct capture into authenticated live Feishin traffic.

| Fixture / host loopback port | Image digest observed with `podman image inspect` |
| --- | --- |
| `docker.io/deluan/navidrome:0.61.0` / 14533 | `sha256:3b26699da946f88cd35be637c6119e72d05651fe04245f7c6c6acff856f8768b` |
| `docker.io/deluan/navidrome:0.47.5` / 14534 | `sha256:4114446451d1122717c33e46339bb9e36ff6568c7c2f92a1a4e008648b49a2bf` |
| `docker.io/sentriz/gonic:v0.16.4` / 14747 | `sha256:c27ac11f457288c4eb86f3ac5c8fddc79655b94f2f0e28cf00f50e60059a1342` |
| `docker.io/sentriz/gonic:v0.15.2` / 14748 | `sha256:70bd4a6d342e58210120ddf7f1fc37582460184d9bc2a4e063d84a8b51e0faee` |
| `docker.io/jellyfin/jellyfin:10.10.7` / 18096 | `sha256:3b38dae4c3ddd6ebc7378538fba4d3f314070ebefbdb3d688166b7c8658fb123` |
| `docker.io/jellyfin/jellyfin:10.8.13` / 18097 | `sha256:887e48b9b7a7a3db741d85bfa7d7914a4ac40e15e0dc1d77275e3416cc2c10e1` |

The old Gonic is a generic Subsonic fixture without advertised OpenSubsonic. Version 0.16.4 advertises OpenSubsonic but only two discovered extensions. Current Navidrome discovers five. Do not assign the same optional features to all servers of a family.

| Observed boundary | Captured result |
| --- | --- |
| Old Navidrome extension discovery | HTTP 404, non-JSON; ordinary authenticated search still works |
| Old Gonic extension discovery | HTTP 200 with Subsonic error 70, `view not found`; ordinary search works |
| Invalid Subsonic authentication | HTTP 200 failed envelope with protocol error 40 |
| Navidrome reader scan | HTTP 200 failed envelope with permission error 50; native user listing returns HTTP 403 |
| Gonic reader scan | Successful response, including on the non-admin account; account role does not imply the same action restrictions across implementations |
| Jellyfin reader | Authentication response verifies `IsAdministrator=false`; item reads succeed, activity-log read returns HTTP 403 on both versions; local system configuration read still returns HTTP 200 |
| Duplicate playlist entries | All four Subsonic-compatible instances preserve two copies of one track, remove index 1 leaving one, then delete the disposable playlist |

The library consists of owned generated 440 Hz/3 s FLAC and 660 Hz/4 s MP3 tones, tagged as Fixture Artist / Fixture Album / P00 Tone A and B, under an artist/album directory. Gonic's scan did not expose tracks placed directly at the music root; moving to that layout and rescanning fixed the fixture. Missing artwork/optional metadata is therefore real fixture data. There is no live 10k/100k library here; those sizes remain separate synthetic page recipes and headless queue measurements. No claim of all backend operations, codecs, Quick Connect, extension versions, browser streaming or cross-platform coverage is made.

Reproduction uses a private 0700 lab root such as `/tmp/feishin-p00-live`, with separate data/cache directories per instance and shared read-only owned music. Create a rootless Podman network with `podman network create --internal feishin-p00-isolated`; publish each port only on `127.0.0.1`. Run as the host UID via `--userns keep-id --user <uid>:<gid>`, with CPU/pid/memory limits, and use appropriate SELinux labels on these temporary directories. Do not mount any real application profile or music library.

Use Navidrome's `/music` read-only and `/data` writable volumes, `ND_LOGLEVEL=warn`, `ND_ENABLEEXTERNALSERVICES=false`; current Navidrome also uses `ND_ENABLEINSIGHTSCOLLECTOR=false`. Both listen internally on 4533. Jellyfin uses `/media` read-only, `/config` and `/cache` writable and port 8096. Gonic uses port 4747 with verified executable flags:

```text
gonic -music-path /music -db-path /data/gonic.db -cache-path /data/cache -podcast-path /data/podcasts -listen-addr 0.0.0.0:4747 -http-log=false -scan-at-start-enabled -scan-interval 0
```

Pre-create its cache/podcast directories. Gonic 0.16.4 additionally requires an existing playlist directory and `-playlists-path /data/playlists`; 0.15.2 does not support that flag. Request logging is disabled so authenticated query URLs never enter logs. Earlier startup/provisioning mistakes were corrected and the affected bootstrap/authentication/scan/capture checks rerun; no failed setup is represented as a passing fixture.

Bootstrap through an HTTP client that never logs request bodies, auth headers or full authenticated URLs. Store generated admin/reader passwords only in a private 0600 file. The collector expects keys `admin`, `password`, `reader`, `reader_password` and can accept additional private bootstrap tokens, which it also redacts. Use these version-specific setup steps:

1. Navidrome: POST `/auth/createAdmin` with **`username`** and `password`; log in at `/auth/login`. With the returned native bearer token in `X-ND-Authorization`, POST `/api/user` with **`userName`**, `name`, `password`, `isAdmin=false` to create the reader. The two different username spellings matter.
2. Jellyfin: GET `/Startup/User` first to initialize the first account, then POST that route with `Name` and `Password`. POST `/Startup/RemoteAccess` disabling remote access/UPnP, then `/Startup/Complete`. Authenticate with `/Users/AuthenticateByName` and a MediaBrowser client identity. Create the reader via `/Users/New`, set its password via `/Users/<id>/Password`, and confirm its policy is non-admin. Add `/media` with `/Library/VirtualFolders`, a music collection and internet metadata providers disabled. The verified 10.10.7 startup handler is [upstream source](https://github.com/jellyfin/jellyfin/blob/v10.10.7/Jellyfin.Api/Controllers/StartupController.cs).
3. Gonic: after initial database creation, stop the instance and seed/rotate only its disposable SQLite `users` table, then restart. The server does not implement Subsonic `createUser`. Its source stores the password string used for salted auth; bcrypt seeding fails. Verify the version's schema/source, set `is_admin=0` for the reader and keep these generated values private. Source anchors: [initial account](https://github.com/sentriz/gonic/blob/v0.16.4/db/migrations.go), [authentication](https://github.com/sentriz/gonic/blob/v0.16.4/server/ctrlsubsonic/ctrl.go). This is fixture preparation, not an approved application persistence design.
4. Trigger scans and wait until each account sees exactly the two owned tones. The capture command then performs reads and creates/removes only its own disposable playlist.

```sh
python3 tests/reference/p00-live-capture.py /tmp/feishin-p00-live/credentials.json /tmp/feishin-p00-live/capture.json
```

Run from the repository root with all six pre-provisioned instances. The tool fixes the host to loopback, bounds ports/responses/timeouts, follows no redirects, omits authentication request material, and redacts response credentials. Its checks deliberately fail rather than silently treating a missing service/account/library as success. Inspect the redacted output and scan for known secret values before copying into tracked fixtures. A fresh run has different generated IDs/timestamps and is not expected to be byte-identical. Stop all task-owned containers afterward; remove only these disposable resources when no longer needed. Image downloads, server profiles, secrets and audio bytes remain outside the repository and are not release assets.
