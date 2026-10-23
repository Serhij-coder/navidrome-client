#!/usr/bin/env python3
"""Optional native keyboard/menu/clipboard journey on private D-Bus and Xvfb."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

TABLE = "Focus table (arrows, Home/End, Shift for range)"
COPY = "Copy synthetic title"


def owned_display(environment):
    return (environment.get("P01_OWNED_INPUT_DISPLAY") == "1"
            and environment.get("DISPLAY", "").startswith(":")
            and environment.get("DBUS_SESSION_BUS_ADDRESS", "").startswith("unix:"))


def run(binary):
    if not owned_display(os.environ):
        raise RuntimeError("explicit owned display/session required")
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi, Gio, GLib
    import pyatspi
    spec = importlib.util.spec_from_file_location("owned_cleanup", Path(__file__).with_name("orca.py"))
    owned = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owned)
    Atspi.set_timeout(1000, 1000)
    checks = {}
    with owned.child_reaper(), tempfile.TemporaryDirectory(prefix="p01-track-actions-") as temporary:
        root = Path(temporary)
        for name in ("config", "data", "cache", "run"):
            (root / name).mkdir(mode=0o700)
        env = owned.private_environment(root, os.environ)
        bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        activation = {name: value for name, value in env.items() if name.startswith("XDG_") or name == "PULSE_SERVER"}
        bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "UpdateActivationEnvironment",
                      GLib.Variant("(a{ss})", (activation,)), None, Gio.DBusCallFlags.NONE, 3000, None)
        bus.call_sync("org.a11y.Bus", "/org/a11y/bus", "org.freedesktop.DBus.Properties", "Set",
                      GLib.Variant("(ssv)", ("org.a11y.Status", "ScreenReaderEnabled", GLib.Variant("b", True))),
                      None, Gio.DBusCallFlags.NONE, 3000, None)
        children = []

        def nodes():
            result = []
            def visit(node, depth=0):
                if node is None or depth > 16 or len(result) >= 512:
                    return
                result.append(node)
                for child in node:
                    if len(result) >= 512:
                        break
                    visit(child, depth + 1)
            for app in pyatspi.Registry.getDesktop(0):
                if app is not None and "feishin" in (app.name or "").lower():
                    visit(app)
            return result

        def focused(name):
            return any(node.name == name and node.getState().contains(pyatspi.STATE_FOCUSED) for node in nodes())

        def wait(predicate, seconds=3):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                if children and children[0].poll() is not None:
                    raise RuntimeError("app exited during journey")
                if predicate():
                    return True
                time.sleep(0.04)
            return False

        def key(value):
            subprocess.run(["xdotool", "key", "--clearmodifiers", value], env=env,
                           check=True, timeout=3, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def tab_to(name, backwards=False):
            for _ in range(32):
                key("shift+Tab" if backwards else "Tab")
                if wait(lambda: focused(name), 0.15):
                    return True
            return False

        def clipboard_is(expected):
            result = subprocess.run(["xclip", "-selection", "clipboard", "-out"], env=env,
                                    capture_output=True, text=True, timeout=3)
            return result.returncode == 0 and result.stdout == expected

        def result():
            return {"checks": checks, "conclusion": "success" if all(checks.values()) else "failure",
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "scope": "Native release binary, actual AT-SPI focus/entry state and X11 clipboard, synthetic Xvfb keys and private profile; no physical input, screen-reader speech, Wayland/other OS/browser certification or timing budget"}

        try:
            children.append(subprocess.Popen([str(binary.resolve())], env=env, stdout=subprocess.DEVNULL,
                                             stderr=subprocess.DEVNULL, start_new_session=True))
            window = None
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                found = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "^Feishin Rust P01$"],
                                       env=env, capture_output=True, text=True, timeout=3)
                values = found.stdout.splitlines()
                if len(values) == 1:
                    window = values[0]
                    break
                time.sleep(0.1)
            if window is None:
                raise RuntimeError("app window unavailable")
            subprocess.run(["xdotool", "windowfocus", "--sync", window], env=env, check=True, timeout=3)
            if not wait(lambda: any(node.name == TABLE for node in nodes()), 10):
                raise RuntimeError("app accessibility tree unavailable")
            checks["table_entry"] = tab_to(TABLE)
            if not checks["table_entry"]:
                return result()
            key("Down")
            checks["first_track"] = wait(lambda: any(node.name == "Table focus: Track 000000, Album 0. 1 selected." for node in nodes()))
            key("shift+F10")
            checks["keyboard_menu"] = wait(lambda: focused(COPY))
            if not checks["keyboard_menu"]:
                return result()
            key("Return")
            checks["first_copy"] = wait(lambda: clipboard_is("Track 000000"))
            checks["copy_focus_return"] = wait(lambda: focused(TABLE))
            key("shift+Down")
            key("shift+F10")
            if not wait(lambda: focused(COPY)):
                raise RuntimeError("range action unavailable")
            key("Return")
            checks["range_copy"] = wait(lambda: clipboard_is("Track 000001"))
            checks["range_preserved"] = wait(lambda: any(node.name == "Table focus: Track 000001, Album 0. 2 selected." for node in nodes()))
            key("shift+F10")
            if not wait(lambda: focused(COPY)):
                raise RuntimeError("cancel action unavailable")
            key("Escape")
            checks["escape_focus_return"] = wait(lambda: focused(TABLE))
            key("shift+F10")
            checks["immediate_reopen"] = wait(lambda: focused(COPY))
            key("Escape")
            checks["second_escape_return"] = wait(lambda: focused(TABLE))
            key("End")
            key("shift+F10")
            if not wait(lambda: focused(COPY)):
                raise RuntimeError("boundary action unavailable")
            key("Return")
            checks["boundary_copy"] = wait(lambda: clipboard_is("Track 099999"))
            checks["filter_entry"] = tab_to("Filter tracks", backwards=True)
            if not checks["filter_entry"]:
                return result()
            key("ctrl+v")
            def pasted():
                return any(node.name == "Filter tracks" and node.queryText().getText(0, -1) == "Track 099999" for node in nodes())
            checks["clipboard_paste"] = wait(pasted)
            checks["filtered_view"] = wait(lambda: any(node.name == "1 rows, 1 selected" for node in nodes()))
            key("shift+F10")
            time.sleep(0.15)
            checks["text_focus_preserved"] = focused("Filter tracks") and not any(node.name == COPY for node in nodes())
            return result()
        finally:
            owned.cleanup(children)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        raise SystemExit("refusing to overwrite evidence")
    try:
        data = run(args.binary)
        with args.output.open("x") as output:
            json.dump(data, output, indent=2)
            output.write("\n")
    except Exception:
        raise SystemExit("Track-action journey failed; no raw process, clipboard or accessibility payload logged.") from None
    print("Track-action journey " + data["conclusion"] + "; physical/platform gates remain open.")
    raise SystemExit(0 if data["conclusion"] == "success" else 1)
