#!/usr/bin/env python3
"""Optional AT-SPI tree audit on an isolated D-Bus/Xvfb session, not a screen reader certification."""
import argparse
import json
import os
import signal
from pathlib import Path
import subprocess
import time

from gi.repository import Gio, GLib
import pyatspi  # Distribution tool: python3-pyatspi, never an app dependency.


def tree(accessible, nodes, depth=0):
    if depth > 16 or len(nodes) >= 512:
        return
    nodes.append({"role": accessible.getRoleName(), "name": accessible.name})
    for child in accessible:
        if len(nodes) >= 512:
            break
        if child is not None:
            tree(child, nodes, depth + 1)


def run(binary):
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    bus.call_sync("org.a11y.Bus", "/org/a11y/bus", "org.freedesktop.DBus.Properties", "Set",
                  GLib.Variant("(ssv)", ("org.a11y.Status", "ScreenReaderEnabled", GLib.Variant("b", True))),
                  None, Gio.DBusCallFlags.NONE, 10_000, None)
    process = subprocess.Popen([str(binary.resolve())], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               env={**os.environ, "APPIMAGE_EXTRACT_AND_RUN":"1"}, start_new_session=True)
    try:
        deadline = time.monotonic() + 15
        nodes = []
        while time.monotonic() < deadline:
            nodes = []
            desktop = pyatspi.Registry.getDesktop(0)
            for app in desktop:
                if app is not None and "feishin" in (app.name or "").lower():
                    tree(app, nodes)
            if any("Focus table" in (node["name"] or "") for node in nodes):
                break
            if process.poll() is not None:
                raise RuntimeError("app exited before accessibility tree")
            time.sleep(0.2)
        labels = [node["name"] for node in nodes if node["name"]]
        required = {"table_focus": any("Focus table" in name for name in labels),
                    "analysis_control": "Analysis preset" in labels,
                    "filter_label": any(node["name"] == "Filter tracks" and node["role"] in ("text", "entry") for node in nodes)}
        if not all(required.values()):
            raise RuntimeError("required accessibility names absent")
        return {"scope":"P01 Linux AT-SPI tree names on isolated Ubuntu/Xvfb/D-Bus; no Orca speech, physical keyboard, OS accessibility certification or other target implied",
                "checks": required, "nodes": nodes, "node_cap":512}
    finally:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        raise SystemExit("refusing to overwrite evidence")
    try:
        result = run(args.binary)
        with args.output.open("x") as output:
            json.dump(result, output, indent=2)
            output.write("\n")
        print("Isolated AT-SPI tree audit passed; screen-reader/platform gates remain open.")
    except (OSError, RuntimeError, subprocess.SubprocessError, GLib.Error):
        raise SystemExit("AT-SPI audit failed; no process output or private data logged.")
