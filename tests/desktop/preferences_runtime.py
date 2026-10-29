#!/usr/bin/env python3
"""Optional native preference journey on an explicitly owned X11/D-Bus session."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def run(binary, screenshot=None):
    if not (os.environ.get("P02_OWNED_INPUT_DISPLAY") == "1"
            and os.environ.get("DISPLAY", "").startswith(":")
            and os.environ.get("DBUS_SESSION_BUS_ADDRESS", "").startswith("unix:")):
        raise RuntimeError("explicit owned X11 and D-Bus session required")
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi, Gio, GLib
    import pyatspi
    spec = importlib.util.spec_from_file_location("owned_cleanup", Path(__file__).with_name("orca.py"))
    owned = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owned)
    Atspi.set_timeout(1000, 1000)
    checks = {}
    children = []
    with owned.child_reaper(), tempfile.TemporaryDirectory(prefix="p02-preferences-") as temporary:
        root = Path(temporary)
        for name in ("config", "data", "cache", "run", "profile"):
            (root / name).mkdir(mode=0o700)
        env = owned.private_environment(root, os.environ)
        bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        activation = {key: value for key, value in env.items() if key.startswith("XDG_") or key == "PULSE_SERVER"}
        bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "UpdateActivationEnvironment",
                      GLib.Variant("(a{ss})", (activation,)), None, Gio.DBusCallFlags.NONE, 3000, None)
        bus.call_sync("org.a11y.Bus", "/org/a11y/bus", "org.freedesktop.DBus.Properties", "Set",
                      GLib.Variant("(ssv)", ("org.a11y.Status", "ScreenReaderEnabled", GLib.Variant("b", True))),
                      None, Gio.DBusCallFlags.NONE, 3000, None)
        directory = root / "profile/feishin-rust-dev-p02"
        directory.mkdir()
        profile = {"id": "fixture", "name": "Fixture server", "endpoint": "http://192.0.2.1/music", "backend": "navidrome"}
        legacy = directory / "profiles-v1.json"
        original = json.dumps({"schema_version": 1, "data": {
            "profiles": [profile], "current_server": "fixture",
            "preferences": {"language": "en", "follow_system_theme": False}}}).encode()
        legacy.write_bytes(original)
        active = directory / "state-v1.json"

        def nodes():
            found = []
            def visit(node, depth=0):
                if node is None or depth > 16 or len(found) >= 1024:
                    return
                found.append(node)
                for child in node:
                    if len(found) >= 1024:
                        break
                    visit(child, depth + 1)
            for app in pyatspi.Registry.getDesktop(0):
                if app is not None and "feishin" in (app.name or "").lower():
                    visit(app)
            return found

        def named(name, roles=None):
            return next((node for node in nodes() if node.name == name
                         and (roles is None or node.getRoleName() in roles)), None)

        def wait(predicate, seconds=10):
            end = time.monotonic() + seconds
            while time.monotonic() < end:
                value = predicate()
                if value:
                    return value
                time.sleep(0.02)
            raise RuntimeError("owned preference journey timed out")

        def command(*arguments):
            subprocess.run(["xdotool", *arguments], env=env, check=True, timeout=3,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def key(value):
            command("key", "--clearmodifiers", value)

        def click(name, roles=None, double=False):
            node = wait(lambda: named(name, roles))
            rect = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
            command("mousemove", "--sync", str(rect.x + rect.width // 2), str(rect.y + rect.height // 2))
            command("click", "--repeat", "2" if double else "1", "--delay", "40", "1")

        def language():
            node = named("Preferred language", {"text", "entry"})
            return node.queryText().getText(0, -1) if node else None

        def edit_language(value):
            click("Preferred language", {"text", "entry"})
            key("ctrl+a")
            command("type", "--clearmodifiers", value)
            wait(lambda: language() == value)

        def launch(persistent=True):
            args = [str(binary.resolve())]
            if persistent:
                args += ["--profile-root", str(root / "profile")]
            process = subprocess.Popen(args, env=env, stdout=subprocess.DEVNULL,
                                       stderr=subprocess.DEVNULL, start_new_session=True)
            children.append(process)
            def window():
                result = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "^Feishin Rust P01$"],
                                        env=env, capture_output=True, text=True, timeout=3)
                values = result.stdout.splitlines()
                return values[0] if len(values) == 1 else None
            identifier = wait(window, 20)
            command("windowactivate", "--sync", identifier)
            return process

        def close(process):
            key("alt+F4")
            process.wait(timeout=10)
            if process.returncode != 0:
                raise RuntimeError("owned app did not close successfully")

        try:
            children.append(subprocess.Popen(["openbox", "--sm-disable"], env=env,
                                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                             start_new_session=True))
            app = launch()
            wait(lambda: named("Preferences loaded"))
            checks["legacy_startup_read_only"] = not active.exists() and legacy.read_bytes() == original
            checks["profile_summary"] = named("Profiles: 1 | Current: Fixture server") is not None
            checks["startup_language"] = language() == "en"
            checks["startup_appearance"] = not named("Follow system appearance").getState().contains(pyatspi.STATE_CHECKED)
            edit_language("uk")
            click("Follow system appearance")
            click("Save", {"push button"}, double=True)
            wait(lambda: named("Preferences saved"))
            saved = json.loads(active.read_bytes())
            checks["native_save"] = saved["current"]["data"]["profile"]["preferences"] == {
                "language": "uk", "follow_system_theme": True}
            checks["double_save_idempotent"] = saved["current"]["revision"] == 1
            checks["profile_preserved"] = saved["current"]["data"]["profile"]["profiles"] == [profile]
            checks["legacy_unchanged"] = legacy.read_bytes() == original
            click("Reload", {"push button"})
            wait(lambda: named("Preferences loaded"))
            checks["reload"] = language() == "uk"
            if screenshot:
                subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "x11grab", "-i", env["DISPLAY"],
                                "-frames:v", "1", str(screenshot)], env=env, check=True, timeout=10,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            close(app)
            app = launch()
            wait(lambda: named("Preferences loaded"))
            checks["reopen"] = language() == "uk" and named("Follow system appearance").getState().contains(pyatspi.STATE_CHECKED)
            edit_language("de")
            click("Follow system appearance")
            click("Save", {"push button"})
            wait(lambda: named("Preparing save") or named("Saving preferences") or named("Preferences saved"))
            close(app)
            saved = json.loads(active.read_bytes())
            checks["accepted_save_survives_close"] = saved["current"]["data"]["profile"]["preferences"] == {
                "language": "de", "follow_system_theme": False}
            checks["checked_revision"] = saved["current"]["revision"] == 2
            future = b'{"schema_version":999}'
            active.write_bytes(future)
            app = launch()
            wait(lambda: named("Saved data uses an unsupported version. Reload after resolving it."))
            checks["future_save_unavailable"] = (named("Save", {"push button"}) is None
                                                  and named("Save unavailable") is not None)
            close(app)
            checks["future_preserved"] = active.read_bytes() == future
            app = launch(False)
            wait(lambda: named("Filter tracks"))
            checks["default_has_no_preferences"] = named("Preferred language", {"text", "entry"}) is None
            close(app)
            checks["default_no_profile_namespace"] = not (root / "data/feishin-rust-dev-p02").exists()
            return {"checks": checks, "conclusion": "success" if all(checks.values()) else "failure",
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "scope": "Actual AT-SPI state and injected X11 keys on private Xvfb/D-Bus; no physical input, Wayland, browser, translation or playback certification"}
        finally:
            owned.cleanup(children)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--screenshot", type=Path)
    arguments = parser.parse_args()
    if arguments.output.exists() or (arguments.screenshot and arguments.screenshot.exists()):
        raise SystemExit("refusing to overwrite evidence")
    try:
        result = run(arguments.binary, arguments.screenshot)
        with arguments.output.open("x") as output:
            json.dump(result, output, indent=2)
            output.write("\n")
    except Exception:
        raise SystemExit("Owned preference journey failed; no raw profile or accessibility payload logged.") from None
    print("Owned preference journey " + result["conclusion"])
    raise SystemExit(0 if result["conclusion"] == "success" else 1)
