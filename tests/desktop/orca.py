#!/usr/bin/env python3
"""Optional Orca keyboard journey in a disposable D-Bus/Xvfb/audio environment."""
import argparse
from contextlib import contextmanager
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time

TABLE = "Focus table (arrows, Home/End, Shift for range) push button."
FIRST = "Table focus: Track 000000, Album 0. 1 selected."
RANGE = "Table focus: Track 000001, Album 0. 2 selected."
END = "Table focus: Track 099999, Album 9999. 1 selected."
PHRASES = {TABLE, FIRST, RANGE, END, "Filter tracks entry.", "Interface scale slider 1.", "Title label."}
SPEECH = re.compile(r"^\d{2}:\d{2}:\d{2}\.\d+ - SPEECH OUTPUT: '([^']*)'(?: |$)")
LOG_LIMIT = 8 * 1024 * 1024
# Orca 46's CLI opens a buffered debug file. Configure that same logging channel
# with line buffering so an idle final announcement is observable immediately.
# The packaged CLI, event handlers and speech generation run unchanged.
ORCA_ENTRY = ("import sys,runpy;from orca import debug;"
              "debug.debugLevel=debug.LEVEL_ALL;debug.eventDebugLevel=debug.LEVEL_OFF;"
              "debug.debugFile=open(sys.argv.pop(1),'w',buffering=1);"
              "runpy.run_path('/usr/bin/orca',run_name='__main__')")


def utterances(text):
    """Retain exact known synthetic phrases, excluding arbitrary process output."""
    return [match[1] for line in text.splitlines()
            if (match := SPEECH.match(line)) and match[1] in PHRASES]


def read_log(path, offset=0):
    if not path.exists():
        return ""
    if path.stat().st_size > LOG_LIMIT:
        raise RuntimeError("screen-reader log limit")
    with path.open("rb") as handle:
        handle.seek(offset)
        return handle.read(LOG_LIMIT + 1).decode(errors="replace")


@contextmanager
def child_reaper():
    """Adopt orphaned owned descendants so cleanup can reap them on Linux."""
    libc = ctypes.CDLL(None, use_errno=True)
    previous = ctypes.c_int()
    # Linux PR_GET_CHILD_SUBREAPER / PR_SET_CHILD_SUBREAPER. This optional
    # harness is Linux-only; no application code or foreign process is changed.
    if libc.prctl(37, ctypes.byref(previous), 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "cannot read child reaper state")
    if libc.prctl(36, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "cannot enable child reaping")
    try:
        yield
    finally:
        if libc.prctl(36, previous.value, 0, 0, 0) != 0:
            raise OSError(ctypes.get_errno(), "cannot restore child reaper state")


def stop(process):
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass
    # The leader can exit while a descendant ignores TERM. Always force-stop
    # the remaining owned group, then reap its adopted children as well.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait(timeout=3)
    deadline = time.monotonic() + 3
    while True:
        try:
            while os.waitpid(-process.pid, os.WNOHANG)[0]:
                pass
        except ChildProcessError:
            pass
        try:
            os.killpg(process.pid, 0)
        except ProcessLookupError:
            return
        if time.monotonic() >= deadline:
            raise RuntimeError("owned process group still present")
        time.sleep(0.02)


def cleanup(children):
    failed = False
    for process in reversed(children):
        try:
            stop(process)
        except (OSError, subprocess.SubprocessError, RuntimeError):
            failed = True
    if failed:
        raise RuntimeError("owned process cleanup incomplete")


def private_environment(root, inherited):
    env = {**inherited, "APPIMAGE_EXTRACT_AND_RUN": "1", "LC_ALL": "C.UTF-8",
           "XDG_CONFIG_HOME": str(root / "config"), "XDG_DATA_HOME": str(root / "data"),
           "XDG_CACHE_HOME": str(root / "cache"), "XDG_RUNTIME_DIR": str(root / "run"),
           "SPEECHD_ADDRESS": "", "SPEECHD_HOST": "", "SPEECHD_PORT": "",
           "SPEECHD_SOCKET": str(root / "speech.sock"), "PULSE_SERVER": "unix:" + str(root / "pulse.sock")}
    env.pop("WAYLAND_DISPLAY", None) # This journey is explicitly X11/Xvfb.
    return env


def run(binary):
    with child_reaper():
        return run_owned(binary)


def run_owned(binary):
    from gi.repository import Gio, GLib
    children = []
    with tempfile.TemporaryDirectory(prefix="p01-orca-owned-") as temporary:
        root = Path(temporary)
        for name in ("config", "data", "cache", "run", "prefs", "speech-config"):
            (root / name).mkdir(mode=0o700)
        env = private_environment(root, os.environ)
        # The caller must provide a disposable D-Bus session. Its activated
        # accessibility/settings services also receive the private profile.
        bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        activation = {key: env[key] for key in env if key.startswith("XDG_") or key.startswith("SPEECHD_") or key == "PULSE_SERVER"}
        bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "UpdateActivationEnvironment",
                      GLib.Variant("(a{ss})", (activation,)), None, Gio.DBusCallFlags.NONE, 10_000, None)
        bus.call_sync("org.a11y.Bus", "/org/a11y/bus", "org.freedesktop.DBus.Properties", "Set",
                      GLib.Variant("(ssv)", ("org.a11y.Status", "ScreenReaderEnabled", GLib.Variant("b", True))),
                      None, Gio.DBusCallFlags.NONE, 10_000, None)
        # Explicit service addresses avoid autospawning against stale/shared sockets.
        (root / "speech-config/speechd.conf").write_text(
            'AudioOutputMethod "pulse"\nLogLevel 3\nDefaultLanguage "en"\n'
            'AddModule "espeak-ng" "sd_espeak-ng" "/etc/speech-dispatcher/modules/espeak-ng.conf"\n'
            'DefaultModule "espeak-ng"\n')
        log = root / "orca.log"

        def spawn(command):
            process = subprocess.Popen(command, env=env, stdout=subprocess.DEVNULL,
                                       stderr=subprocess.DEVNULL, start_new_session=True)
            children.append(process)
            return process

        def ready(path, process):
            deadline = time.monotonic() + 8
            while not path.exists():
                if process.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError("owned service unavailable")
                time.sleep(0.05)

        def key(value):
            if any(child.poll() is not None for child in children):
                raise RuntimeError("owned process exited")
            subprocess.run(["xdotool", "key", "--clearmodifiers", value],
                           check=True, timeout=3, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def step(value, expected):
            offset = log.stat().st_size
            key(value)
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                if expected in utterances(read_log(log, offset)):
                    return True
                time.sleep(0.05)
            return False

        try:
            pulse = spawn(["pulseaudio", "-n", "--daemonize=no", "--exit-idle-time=-1",
                           "--load=module-null-sink sink_name=p01_speech",
                           "--load=module-native-protocol-unix socket=" + str(root / "pulse.sock") + " auth-anonymous=1"])
            ready(root / "pulse.sock", pulse)
            dispatcher = spawn(["speech-dispatcher", "-s", "-S", str(root / "speech.sock"),
                                "-C", str(root / "speech-config"), "-L", str(root), "-P", str(root / "speech.pid"), "-t", "0"])
            ready(root / "speech.sock", dispatcher)
            reader = spawn(["/usr/bin/python3", "-c", ORCA_ENTRY, str(log),
                            "--user-prefs", str(root / "prefs"), "--disable", "braille"])
            ready(log, reader)
            time.sleep(2)
            spawn([str(binary.resolve())])
            deadline = time.monotonic() + 15
            window = None
            while time.monotonic() < deadline:
                result = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "^Feishin Rust P01$"],
                                        capture_output=True, text=True, timeout=3)
                windows = result.stdout.splitlines()
                if len(windows) == 1:
                    window = windows[0]
                    break
                time.sleep(0.1)
            if window is None:
                raise RuntimeError("app window unavailable")
            subprocess.run(["xdotool", "windowfocus", "--sync", window], check=True, timeout=3)
            for _ in range(32):
                key("Tab")
                time.sleep(0.4)
                if TABLE in utterances(read_log(log)):
                    break
            spoken = utterances(read_log(log))
            checks = {"filter_name": "Filter tracks entry." in spoken,
                      "scale_name": "Interface scale slider 1." in spoken,
                      "table_entry": TABLE in spoken}
            for name, value, expected in [("first", "Down", FIRST), ("range", "shift+Down", RANGE),
                                           ("end", "End", END), ("home", "Home", FIRST),
                                           ("exit", "Tab", "Title label."), ("return", "shift+Tab", TABLE)]:
                checks[name] = step(value, expected)
            return {"checks": checks, "conclusion": "success" if all(checks.values()) else "failure",
                    "utterances": sorted(set(utterances(read_log(log)))),
                    "orca_version": subprocess.check_output(["orca", "--version"], text=True, timeout=3).strip(),
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "scope": "Orca requested announcements with Speech Dispatcher/eSpeak NG on an isolated null sink and synthetic X11 keys; not physical keyboard, human voice evaluation or platform certification"}
        finally:
            cleanup(children)


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
    except Exception:
        raise SystemExit("Orca harness failed; no raw process output or private data logged.") from None
    print("Orca keyboard journey " + result["conclusion"] + "; physical/platform gates remain open.")
    raise SystemExit(0 if result["conclusion"] == "success" else 1)
