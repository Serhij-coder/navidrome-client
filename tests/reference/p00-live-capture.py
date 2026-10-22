#!/usr/bin/env python3
"""Capture redacted protocol evidence from pre-provisioned disposable P00 servers."""
import argparse
import hashlib
import http.client
import json
from pathlib import Path
import secrets
import time
from urllib.parse import urlencode


SENSITIVE = {"password", "pw", "currentpw", "newpw", "token", "accesstoken", "subsonictoken", "subsonicsalt",
             "authorization", "xembytoken", "xndauthorization", "credential", "apikey", "salt"}


def redact(value, secret_values):
    if isinstance(value, dict):
        return {key: "<redacted>" if "".join(c for c in key.lower() if c.isalnum()) in SENSITIVE
                else redact(item, secret_values) for key, item in value.items()}
    if isinstance(value, list):
        return [redact(item, secret_values) for item in value]
    if isinstance(value, str):
        for secret in sorted((s for s in secret_values if s), key=len, reverse=True):
            value = value.replace(secret, "<redacted>")
    return value


class FixtureError(ValueError):
    """A safe diagnostic assembled only from fixed fixture labels."""


class Capture:
    def __init__(self, credentials):
        self.credentials = credentials
        self.secrets = [v for key, v in credentials.items() if "password" in key or "token" in key]
        self.records = []

    def call(self, fixture, port, path, query=None, body=None, headers=None, method=None, auth=None, record=True):
        # Explicitly fixed loopback host, no proxies/redirect following or arbitrary destination.
        if port not in {14533, 14534, 14747, 14748, 18096, 18097} or not path.startswith("/") or "?" in path:
            raise FixtureError("Only configured loopback fixture ports and query-free paths are allowed")
        query = query or {}
        url = path + ("?" + urlencode(query | (auth or {}), doseq=True) if query or auth else "")
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=20)
        try:
            connection.request(method or ("POST" if body is not None else "GET"), url,
                               json.dumps(body).encode() if body is not None else None,
                               {"Content-Type": "application/json", **(headers or {})})
            response = connection.getresponse()
            raw = response.read(4 * 1024 * 1024 + 1)
            if len(raw) > 4 * 1024 * 1024:
                raise FixtureError("fixture response exceeded 4 MiB limit")
            try:
                data = json.loads(raw) if raw else None
            except ValueError:
                data = {"non_json": True, "bytes": len(raw)}
            if record:
                self.records.append(redact({"fixture": fixture, "method": method or ("POST" if body is not None else "GET"),
                                            "path": path, "query": query, "http_status": response.status, "body": data}, self.secrets))
            return response.status, data
        finally:
            connection.close()

    def subsonic(self, fixture, port, role, endpoint, query=None, invalid=False):
        c = self.credentials
        username = ("admin" if port in {14747, 14748} else c["admin"]) if role == "admin" else c["reader"]
        password = c["password"] if role == "admin" else c["reader_password"]
        salt = secrets.token_hex(6)
        token = hashlib.md5((password + salt).encode()).hexdigest() if not invalid else "invalid"
        self.secrets.append(salt)
        if not invalid:
            self.secrets.append(token)
        status, body = self.call(fixture + ":" + role, port, "/rest/" + endpoint, query,
                                 auth={"u": username, "s": salt, "t": token, "v": "1.16.1", "c": "P00", "f": "json"})
        return status, (body or {}).get("subsonic-response", {})

    def delete_playlist(self, fixture, port, playlist_id):
        status, response = self.subsonic(fixture, port, "admin", "deletePlaylist", {"id": playlist_id})
        if not 200 <= status < 300 or response.get("status") != "ok":
            # Do not include response bodies or request values in diagnostics.
            raise FixtureError("Disposable playlist cleanup failed; capture is incomplete")


def capture(credentials):
    h = Capture(credentials)
    summaries = []
    for fixture, port in [("navidrome-0.61.0", 14533), ("navidrome-0.47.5", 14534), ("gonic-0.16.4", 14747), ("gonic-0.15.2", 14748)]:
        _, ping = h.subsonic(fixture, port, "admin", "ping")
        if ping.get("status") != "ok":
            raise FixtureError(f"{fixture}: fixture admin authentication failed")
        extension_status, extensions = h.subsonic(fixture, port, "admin", "getOpenSubsonicExtensions")
        h.subsonic(fixture, port, "admin", "ping", invalid=True)
        h.subsonic(fixture, port, "admin", "startScan")
        songs = []
        for attempt in range(20):
            _, search = h.subsonic(fixture, port, "admin", "search3", {"query": "P00", "songCount": 20, "artistCount": 0, "albumCount": 0})
            songs = search.get("searchResult3", {}).get("song", [])
            if len(songs) >= 2:
                break
            time.sleep(0.25)
        if len(songs) != 2:
            raise FixtureError(f"{fixture}: expected two owned test tones after scan")
        h.subsonic(fixture, port, "admin", "search3", {"query": "P00", "songOffset": 2, "songCount": 20})
        playlist_id = None
        try:
            _, created = h.subsonic(fixture, port, "admin", "createPlaylist", {"name": "P00 disposable duplicates", "songId": [songs[0]["id"], songs[0]["id"]]})
            playlist_id = created.get("playlist", {}).get("id")
            if not playlist_id:
                raise FixtureError(f"{fixture}: disposable playlist creation failed")
            _, playlist = h.subsonic(fixture, port, "admin", "getPlaylist", {"id": playlist_id})
            if len(playlist.get("playlist", {}).get("entry", [])) != 2:
                raise FixtureError(f"{fixture}: duplicate playlist entries were not preserved")
            h.subsonic(fixture, port, "admin", "updatePlaylist", {"playlistId": playlist_id, "songIndexToRemove": 1})
            _, after = h.subsonic(fixture, port, "admin", "getPlaylist", {"id": playlist_id})
            if len(after.get("playlist", {}).get("entry", [])) != 1:
                raise FixtureError(f"{fixture}: removal did not preserve one duplicate occurrence")
        finally:
            if playlist_id:
                h.delete_playlist(fixture, port, playlist_id)
        _, reader = h.subsonic(fixture, port, "reader", "ping")
        reader_status = reader.get("status")
        if reader_status != "ok":
            raise FixtureError(f"{fixture}: fixture reader authentication failed")
        h.subsonic(fixture, port, "reader", "search3", {"query": "P00", "songCount": 20})
        h.subsonic(fixture, port, "reader", "startScan")
        summaries.append({"fixture": fixture, "serverVersion": ping.get("serverVersion"), "protocolVersion": ping["version"],
                          "openSubsonicAdvertised": ping.get("openSubsonic", False), "extensionHttpStatus": extension_status,
                          "extensionProtocolStatus": extensions.get("status"), "songs": len(songs), "readerAuthentication": reader_status})
        if port not in {14747, 14748}:
            status, login = h.call(fixture, port, "/auth/login", body={"username": credentials["admin"], "password": credentials["password"]}, record=False)
            if status != 200:
                raise FixtureError(f"{fixture}: native login failed")
            if login.get("isAdmin") is not True:
                raise FixtureError(f"{fixture}: native admin role mismatch")
            token = login["token"]; h.secrets.append(token)
            h.records.append(redact({"fixture": fixture + ":admin", "method": "POST", "path": "/auth/login", "query": {}, "http_status": status, "body": login}, h.secrets))
            h.call(fixture + ":admin", port, "/api/song", {"_start": 0, "_end": 20}, headers={"X-ND-Authorization": "Bearer " + token})
            status, login = h.call(fixture, port, "/auth/login", body={"username": credentials["reader"], "password": credentials["reader_password"]}, record=False)
            if status != 200:
                raise FixtureError("Navidrome native reader login failed")
            if login.get("isAdmin") is not False:
                raise FixtureError("Navidrome native reader role mismatch")
            token = login["token"]; h.secrets.append(token)
            h.records.append(redact({"fixture": fixture + ":reader", "method": "POST", "path": "/auth/login", "query": {}, "http_status": status, "body": login}, h.secrets))
            h.call(fixture + ":reader", port, "/api/user", headers={"X-ND-Authorization": "Bearer " + token})

    for fixture, port in [("jellyfin-10.10.7", 18096), ("jellyfin-10.8.13", 18097)]:
        _, public_info = h.call(fixture, port, "/System/Info/Public")
        for role in ["admin", "reader"]:
            status, login = h.call(fixture, port, "/Users/AuthenticateByName", body={"Username": credentials[role], "Pw": credentials["password" if role == "admin" else "reader_password"]},
                                   headers={"Authorization": 'MediaBrowser Client="P00", Device="fixture", DeviceId="p00", Version="1"'}, record=False)
            if status != 200:
                raise FixtureError(f"Jellyfin {role} authentication failed")
            if bool(login["User"]["Policy"]["IsAdministrator"]) != (role == "admin"):
                raise FixtureError(f"Jellyfin {role}: fixture role mismatch")
            token = login["AccessToken"]; h.secrets.append(token)
            h.records.append(redact({"fixture": fixture + ":" + role, "method": "POST", "path": "/Users/AuthenticateByName", "query": {}, "http_status": status, "body": login}, h.secrets))
            user_id = login["User"]["Id"]
            headers = {"X-Emby-Token": token}
            _, items = h.call(fixture + ":" + role, port, "/Users/" + user_id + "/Items",
                              {"Recursive": "true", "IncludeItemTypes": "Audio", "Fields": "MediaSources,DateCreated,Genres,ParentId,Path", "Limit": 20}, headers=headers)
            if len((items or {}).get("Items", [])) != 2:
                raise FixtureError(f"Jellyfin {role}: expected two owned test tones")
            h.call(fixture + ":" + role, port, "/Users/" + user_id + "/Items", {"Recursive": "true", "IncludeItemTypes": "Audio", "StartIndex": 2}, headers=headers)
            h.call(fixture + ":" + role, port, "/System/Info", headers=headers)
            h.call(fixture + ":" + role, port, "/System/Configuration", headers=headers)
            h.call(fixture + ":" + role, port, "/System/ActivityLog/Entries", {"Limit": 1}, headers=headers)
        h.call(fixture, port, "/System/Info", headers={"X-Emby-Token": "invalid"})
        summaries.append({"fixture": fixture, "serverVersion": public_info["Version"], "songs": 2, "readerAuthentication": "ok", "readerIsAdministrator": False})
    return {"schema": 1, "task": "P00.04", "evidenceKind": "live-disposable-server-http-capture", "date": time.strftime("%Y-%m-%d", time.gmtime()),
            "summaries": summaries, "records": h.records,
            "limitations": ["Direct server HTTP API probes; no pinned Feishin controller, UI or audio/device execution.",
                            "Two owned test tones; 10k/100k synthetic recipes are separate and not live large-library evidence.",
                            "This is a six-instance Linux/x86_64 matrix, not all server implementations or platforms.",
                            "Gonic accounts seeded in stopped disposable database because createUser is unsupported."]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path, help="private /tmp fixture credentials file; never tracked")
    parser.add_argument("output", type=Path, help="redacted output JSON")
    args = parser.parse_args()
    if args.credentials.stat().st_mode & 0o077:
        parser.error("fixture credentials file must have private permissions (0600)")
    try:
        report = capture(json.loads(args.credentials.read_text()))
    except FixtureError as error:
        parser.exit(1, f"Fixture capture failed: {error}\n")
    except (OSError, ValueError, KeyError, http.client.HTTPException) as error:
        # Never include exception URLs or bodies, which can contain credentials.
        parser.exit(1, f"Fixture capture failed ({type(error).__name__}); output was not written.\n")
    args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(f"Saved {len(report['records'])} redacted fixture responses")


if __name__ == "__main__":
    main()
