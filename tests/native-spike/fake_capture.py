#!/usr/bin/env python3
"""Disposable deterministic PCM/failed-source peer; never a host audio source."""
import math
import os
from pathlib import Path
import struct
import sys
import time

executable = Path(sys.argv[0])
with executable.with_suffix('.starts').open('a') as output:
    output.write(str(os.getpid()) + '\n')
if executable.name == 'failed-source.py':
    sys.exit(1)
samples = b''.join(struct.pack('<f', math.sin(i * math.tau * 4 / 256)) for i in range(256))
while True:
    sys.stdout.buffer.write(samples)
    sys.stdout.buffer.flush()
    time.sleep(0.032)
