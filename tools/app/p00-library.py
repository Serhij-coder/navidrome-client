#!/usr/bin/env python3
"""Emit deterministic synthetic wire pages; no server or audio parity implied."""

import argparse
import json


def page(backend, count, offset, limit):
    if backend not in {"jellyfin", "navidrome", "subsonic"}:
        raise ValueError("unsupported backend")
    if not 0 <= count <= 100_000 or not 0 <= offset <= 100_000 or not 1 <= limit <= 500:
        raise ValueError("count/offset must be 0..100000 and limit 1..500")
    items = []
    for index in range(offset, min(count, offset + limit)):
        song_id = f"song-{index:06d}"
        album_id = f"album-{index // 10:05d}"
        title = f"Track {index:06d} - 日本語" if index % 17 == 0 else f"Track {index:06d}"
        if backend == "jellyfin":
            item = {"Id": song_id, "Name": title, "Type": "Audio", "AlbumId": album_id,
                    "Album": album_id, "RunTimeTicks": 1_800_000_000,
                    "Artists": ["Fixture artist"], "ArtistItems": [], "AlbumArtists": [],
                    "MediaSources": [], "AlbumArtist": "Fixture artist", "AlbumPrimaryImageTag": "",
                    "BackdropImageTags": [], "ChannelId": None, "DateCreated": "2020-01-01T00:00:00Z",
                    "ExternalUrls": [], "GenreItems": [], "Genres": [], "ImageBlurHashes": {},
                    "ImageTags": {}, "IndexNumber": index % 10 + 1, "IsFolder": False,
                    "LocationType": "FileSystem", "MediaType": "Audio", "ParentIndexNumber": 1,
                    "ProductionYear": 2020, "ServerId": "fixture-jellyfin"}
        elif backend == "navidrome":
            item = {"id": song_id, "title": title, "albumId": album_id, "album": album_id,
                    "artist": "Fixture artist", "artistId": "artist-1", "albumArtist": "Fixture artist",
                    "duration": 180, "suffix": "flac", "bitRate": 1000, "size": 1000000,
                    "trackNumber": index % 10 + 1, "discNumber": 1, "starred": False,
                    "albumArtistId": "artist-1", "bookmarkPosition": 0, "compilation": False,
                    "createdAt": "2020-01-01T00:00:00Z", "genre": "", "genres": [],
                    "hasCoverArt": False, "orderAlbumArtistName": "Fixture artist", "orderAlbumName": album_id,
                    "orderArtistName": "Fixture artist", "orderTitle": title, "path": f"/fixture/{song_id}.flac",
                    "sampleRate": 44100, "updatedAt": "2020-01-01T00:00:00Z", "year": 2020}
        else:
            item = {"id": song_id, "title": title, "albumId": album_id, "album": album_id,
                    "artist": "Fixture artist", "artistId": "artist-1", "duration": 180,
                    "contentType": "audio/flac", "suffix": "flac", "isDir": False,
                    "created": "2020-01-01T00:00:00Z", "track": index % 10 + 1,
                    "albumArtists": [], "artists": [], "discNumber": 1, "isVideo": False,
                    "parent": album_id, "path": f"/fixture/{song_id}.flac", "size": 1000000, "type": "music"}
        items.append(item)
    if backend == "jellyfin":
        return {"Items": items, "StartIndex": offset, "TotalRecordCount": count}
    if backend == "navidrome":
        return items  # Native totals travel in HTTP headers, not an invented JSON envelope.
    return {"subsonic-response": {"status": "ok", "version": "1.16.1", "searchResult3": {"song": items}}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("backend", choices=["jellyfin", "navidrome", "subsonic"])
    parser.add_argument("--count", type=int, choices=[0, 10, 10_000, 100_000], default=10)
    parser.add_argument("--offset", type=int, default=0)
    parser.add_argument("--limit", type=int, default=500)
    args = parser.parse_args()
    try:
        data = page(args.backend, args.count, args.offset, args.limit)
    except ValueError as error:
        parser.error(str(error))
    print(json.dumps(data, ensure_ascii=False, separators=(",", ":")))


if __name__ == "__main__":
    main()
