#!/usr/bin/env python3
"""Disposable IPC peer: protocol races and process teardown, never real playback."""
import json
from pathlib import Path
import socket
import sys
import time

mode = Path(sys.argv[0]).name
path = next(arg.split("=", 1)[1] for arg in sys.argv[1:] if arg.startswith("--input-ipc-server="))
listener = socket.socket(socket.AF_UNIX)
listener.bind(path)
listener.listen(1)
connection, _ = listener.accept()
stream = connection.makefile("rwb", buffering=0)
count = 0
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
    if command[0] == "loadfile":
        count = count + 1 if command[2] == "append" else 1
    data = "fixture-version" if command == ["get_property", "mpv-version"] else count
    stream.write(json.dumps({"request_id": request.get("request_id"), "error": "success", "data": data}).encode() + b"\n")
