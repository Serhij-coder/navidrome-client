#!/usr/bin/env python3
"""Evaluate existing FFmpeg filters with owned files, not a live audio engine.

No network, profile or credential input. Output records offline PCM/processing
measurements; this cannot certify device gapless/crossfade or dynamic controls.
"""
import argparse
from array import array
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import wave

RATE = 48_000
SECONDS = 12


def tone(path, frequency):
    pcm = array("h", (round(math.sin(i * math.tau * frequency / RATE) * 8192)
                      for i in range(RATE * SECONDS)))
    if sys.byteorder != "little":
        pcm.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setparams((1, 2, RATE, 0, "NONE", "not compressed"))
        output.writeframes(pcm.tobytes())


def decode(binary, inputs, graph):
    args = [str(binary), "-nostdin", "-hide_banner", "-loglevel", "error"]
    for file in inputs:
        args.extend(("-i", str(file)))
    args.extend(("-filter_complex", graph, "-map", "[out]", "-ac", "1", "-ar", str(RATE), "-f", "f32le", "pipe:1"))
    start = time.perf_counter()
    result = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=20, check=True)
    elapsed = (time.perf_counter() - start) * 1000
    if not result.stdout or len(result.stdout) % 4 or len(result.stdout) > RATE * 4 * 50:
        raise ValueError("unexpected PCM length")
    pcm = array("f")
    pcm.frombytes(result.stdout)
    if sys.byteorder != "little":
        pcm.byteswap()
    if any(not math.isfinite(value) for value in pcm):
        raise ValueError("invalid PCM")
    return pcm, elapsed


def envelope(pcm, bins=256):
    stride = math.ceil(len(pcm) / bins)
    return [max(abs(value) for value in pcm[start:start + stride])
            for start in range(0, len(pcm), stride)]


def run(binary, samples):
    version = subprocess.run([str(binary), "-version"], capture_output=True, text=True, timeout=10, check=True).stdout.splitlines()[0]
    cases = []
    with tempfile.TemporaryDirectory(prefix="feishin-p01-filters-") as temporary:
        paths = [Path(temporary) / f"tone-{frequency}.wav" for frequency in (440, 660)]
        for path, frequency in zip(paths, (440, 660)):
            tone(path, frequency)
        graphs = [("concat", 1.0, 0, "none", "[0:a][1:a]concat=n=2:v=0:a=1[out]")]
        for speed in (0.5, 1.0, 2.0):
            for fade in (1, 5):
                for curve in ("tri", "qsin"):
                    graphs.append(("crossfade", speed, fade, curve,
                                   f"[0:a]atempo={speed}[a];[1:a]atempo={speed}[b];[a][b]acrossfade=d={fade}:c1={curve}:c2={curve}[out]"))
        for kind, speed, fade, curve, graph in graphs:
            durations, costs = [], []
            for _ in range(samples):
                pcm, elapsed = decode(binary, paths, graph)
                durations.append(len(pcm) / RATE)
                costs.append(elapsed)
            cases.append({"kind": kind, "speed": speed, "fade_seconds": fade, "ffmpeg_curve": curve,
                          "ideal_duration_seconds": SECONDS * 2 / speed - fade,
                          "observed_duration_seconds": durations, "processing_ms": costs})
        # Lowpass proves an existing native filter and bounded waveform readiness.
        filtered, elapsed = decode(binary, [paths[0]], "[0:a]lowpass=f=2000[out]")
        waveform = envelope(filtered)
        if len(waveform) > 256 or max(waveform) < 0.01:
            raise ValueError("waveform readiness failed")
    return {"schema": 1, "scope": "P01.05 offline FFmpeg library/filter evaluation; not live engine scheduling, audio-device timing, authenticated streaming, native fallback selection or P00 budget acceptance",
            "ffmpeg": version, "sample_rate_hz": RATE, "owned_input_seconds_each": SECONDS,
            "curves": {"tri": "linear", "qsin": "quarter sine"}, "cases": cases,
            "filtered_waveform": {"bins": waveform, "ready_after_decode_ms": elapsed},
            "limits": ["No dynamic pause/seek/device control", "Offline concat is not measured mpv gapless", "FFmpeg curves are candidates; no equivalence to upstream fade styles asserted", "CPU-only offline processing; no concurrent renderer/audio-device cost"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ffmpeg", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=30, choices=range(1, 31))
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise SystemExit("refusing to overwrite evidence")
    result = run(args.ffmpeg, args.samples)
    with args.out.open("x") as output:
        json.dump(result, output, indent=2)
        output.write("\n")
    print("Offline native filter/waveform evaluation recorded; live engine gates remain open.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError):
        raise SystemExit("Native filter experiment failed; no paths or child output logged.")
