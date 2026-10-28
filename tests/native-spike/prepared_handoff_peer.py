#!/usr/bin/env python3
"""Owned prepared-source peer. File gates control observations, never audio."""
import json
import os
from pathlib import Path
import socket
import sys

exe = Path(sys.argv[0])
directory, name = exe.parent, exe.name


def record(event, command=None):
    with (directory / 'events.jsonl').open('a') as output:
        output.write(json.dumps(dict(name=name, event=event, pid=os.getpid(), command=command)) + '\n')


for marker in directory.glob('pid-*'):
    try:
        os.kill(int(marker.read_text()), 0)
    except ProcessLookupError:
        continue
    record('overlap')
(directory / ('pid-' + name)).write_text(str(os.getpid()))
record('started')
path = next(a.split('=', 1)[1] for a in sys.argv if a.startswith('--input-ipc-server='))
listener = socket.socket(socket.AF_UNIX)
listener.bind(path)
listener.listen(1)
connection, _ = listener.accept()
stream = connection.makefile('rwb', buffering=0)
paused, volume, loaded, target = False, 100.0, False, None
for line in stream:
    request = json.loads(line)
    cmd = request['command']
    record('command', cmd)
    error, data = 'success', None
    if cmd[0] == 'quit':
        record('exited')
        break
    if cmd[0] == 'loadfile':
        loaded = True
        if name.startswith('reject'):
            error = 'loading failed'
    elif cmd[:2] == ['set_property', 'pause']:
        paused = cmd[2]
    elif cmd[:2] == ['set_property', 'volume']:
        volume = cmd[2]
    elif cmd[0] == 'seek':
        target = cmd[1]
        (directory / ('seek-' + name)).touch()
    elif cmd[0] == 'get_property':
        prop = cmd[1]
        if prop == 'mpv-version':
            data = 'owned-preparation-peer'
        elif prop == 'time-pos':
            (directory / ('position-' + name)).touch()
            if not loaded or name.startswith('ready') and not (directory / ('release-ready-' + name)).exists():
                error = 'property unavailable'
            elif target is not None:
                data = target if not name.startswith(('seekgate', 'noarrival')) or (directory / ('release-seek-' + name)).exists() else 0.0
            else:
                data = 0.0
        elif prop == 'idle-active':
            data = not loaded or name.startswith('mismatch')
        elif prop == 'pause':
            data = paused
        elif prop == 'volume':
            data = 100.0 if name.startswith('wrongvolume') else volume
        elif prop == 'duration':
            data = 20.0
    stream.write(json.dumps(dict(request_id=request.get('request_id'), error=error, data=data)).encode() + b'\n')
