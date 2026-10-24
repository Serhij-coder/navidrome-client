#!/usr/bin/env python3
"""Disposable IPC peer: protocol races and process teardown, never real playback."""
import json
from pathlib import Path
import socket
import sys
import time

mode = Path(sys.argv[0]).name
if mode == "ffplay-stall":
    time.sleep(60)
    sys.exit(0)
path = next(arg.split("=", 1)[1] for arg in sys.argv[1:] if arg.startswith("--input-ipc-server="))
listener = socket.socket(socket.AF_UNIX)
listener.bind(path)
listener.listen(1)
connection, _ = listener.accept()
stream = connection.makefile("rwb", buffering=0)
count = 0
position = 0.0
paused = False
volume = 100.0
pending_position = None
unavailable_queries = 0
seek_queries = 0
for line in stream:
    request = json.loads(line)
    command = request["command"]
    if mode == "slow-start" or mode == "slow-drip" and command != ["get_property", "mpv-version"]:
        for _ in range(50):
            stream.write(b"x")
            time.sleep(0.1)
        time.sleep(60)
        continue
    if command[0] == "quit":
        if mode == "stalled-quit":
            time.sleep(60)
        break
    if command != ["get_property", "mpv-version"]:
        if mode == "die":
            sys.exit(0)
        if mode == "malformed":
            stream.write(b"{invalid\n")
            continue
        if mode == "oversized":
            stream.write(b"x" * 65537 + b"\n")
            continue
        if mode == "wrong-id":
            stream.write(b'{"request_id":9999,"error":"success"}\n')
            time.sleep(60)
            continue
    if command[0] == "get_property" and command[1] in ["idle-active", "duration", "volume"]:
        property_name = command[1]
        fault = {"mpris-invalid-idle": "idle-active", "mpris-invalid-duration": "duration", "mpris-invalid-volume": "volume"}.get(mode)
        unavailable = {"mpris-unavailable-idle": "idle-active", "mpris-unavailable-duration": "duration"}.get(mode)
        if fault == property_name or unavailable == property_name:
            stream.write(json.dumps({"request_id": request.get("request_id"), "error": "property unavailable" if unavailable else "success", "data": "invalid-private-dto"}).encode() + b"\n")
            continue
    if command == ["get_property", "idle-active"]:
        stream.write(json.dumps({"request_id": request.get("request_id"), "error": "success", "data": mode == "mpris-idle" or count == 0}).encode() + b"\n")
        continue
    if command[0] == "loadfile":
        count = count + 1 if command[2] == "append" else 1
        unavailable_queries = 2
    if command[:2] == ["set_property", "volume"]:
        volume = command[2]
    if command in [["get_property", "volume"], ["get_property", "duration"]]:
        data = volume if command[1] == "volume" else 10.0
        stream.write(json.dumps({"request_id": request.get("request_id"), "error": "success", "data": data}).encode() + b"\n")
        continue
    if command[:2] == ["set_property", "pause"]:
        paused = command[2]
    if command == ["get_property", "pause"]:
        stream.write(json.dumps({"request_id": request.get("request_id"), "error": "success", "data": paused}).encode() + b"\n")
        continue
    if command[0] == "seek":
        pending_position = command[1]
        if mode == "mpris-stateful":
            position = pending_position
        seek_queries = 3
    if command == ["get_property", "time-pos"]:
        error = "success"
        if mode == "slow-position":
            for _ in range(100):
                stream.write(b" ")
                time.sleep(0.02)
        if mode in ["unavailable-position", "mpris-loading"] or mode == "delayed-position" and (count == 0 or unavailable_queries > 0):
            error = "property unavailable"
            unavailable_queries -= 1
        elif mode == "rejected-position":
            error = "property not found"
        elif mode == "delayed-position" and pending_position is not None:
            if seek_queries > 0:
                seek_queries -= 1
            else:
                position = pending_position
        data = {"negative-position": -1, "null-position": None, "string-position": "private-invalid-position"}.get(mode, position)
        stream.write(json.dumps({"request_id": request.get("request_id"), "error": error, "data": data}).encode() + b"\n")
        continue
    data = "fixture-version" if command == ["get_property", "mpv-version"] else count
    stream.write(json.dumps({"request_id": request.get("request_id"), "error": "success", "data": data}).encode() + b"\n")
