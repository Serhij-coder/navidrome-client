#!/usr/bin/env python3
"""Owned Unix IPC lifecycle peer, with explicit test gates and overlap detection."""
import json
import os
from pathlib import Path
import socket
import sys
import time

executable = Path(sys.argv[0])
directory = executable.parent
name = executable.name


def record(event):
    with (directory / "events.jsonl").open("a") as output:
        output.write(json.dumps({"name": name, "event": event, "pid": os.getpid()}) + "\n")


for marker in directory.glob("pid-*"):
    try:
        os.kill(int(marker.read_text()), 0)
    except ProcessLookupError:
        pass
    else:
        record("overlap")
(directory / ("pid-" + name)).write_text(str(os.getpid()))
record("started")
(directory / ("started-" + name)).touch()
if name.startswith("gate-start"):
    deadline = time.monotonic() + 10
    while not (directory / ("release-" + name)).exists():
        if time.monotonic() >= deadline:
            sys.exit(2)
        time.sleep(0.002)
path = next(arg.split("=", 1)[1] for arg in sys.argv[1:] if arg.startswith("--input-ipc-server="))
listener = socket.socket(socket.AF_UNIX)
listener.bind(path)
listener.listen(1)
connection, _ = listener.accept()
stream = connection.makefile("rwb", buffering=0)
paused = False
for line in stream:
    request = json.loads(line)
    command = request["command"]
    if command[0] == "quit":
        record("quit")
        if name.startswith("stalled-quit"):
            time.sleep(60)
        break
    if name.startswith("die-command") and command != ["get_property", "mpv-version"]:
        record("died")
        sys.exit(0)
    if command[:2] == ["set_property", "pause"]:
        paused = command[2]
    if name.startswith("stall-position") and command == ["get_property", "time-pos"]:
        time.sleep(60)
    data = paused if command == ["get_property", "pause"] else 0.0
    if command == ["get_property", "mpv-version"]:
        data = "handoff-fixture-version"
    response = {"request_id": request.get("request_id"), "error": "success", "data": data}
    stream.write(json.dumps(response).encode() + b"\n")
record("exited")
