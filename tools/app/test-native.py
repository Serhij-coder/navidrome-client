#!/usr/bin/env python3
"""Run tracked native tests and expose failed test names as CI annotations.

Public check annotations remain readable when detailed hosted logs need login.
Only Rust test identifiers enter annotations; assertion data/payloads do not.
"""
import argparse
import os
import re
import subprocess
import sys

FAILED = re.compile(r"^test ([A-Za-z0-9_:]+) \.\.\. FAILED$")


def annotation(line):
    match = FAILED.fullmatch(line.strip())
    return f"::error title=Native Rust regression::{match[1]} failed; inspect the authenticated job log for assertion details" if match else None


def run(command):
    process = subprocess.Popen(command,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    assert process.stdout is not None  # PIPE invariant, not external input.
    for line in process.stdout:
        print(line, end="", flush=True)
        if os.environ.get("GITHUB_ACTIONS") == "true" and (message := annotation(line)):
            print(message, flush=True)
    return process.wait()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--external-audio", action="store_true",
                      help="Run ignored native probes with an explicitly provisioned owned PulseAudio sink")
    mode.add_argument("--native-controls", action="store_true",
                      help="Run the three ignored mpv routing, handoff and owned-bus control probes")
    mode.add_argument("--gstreamer-probe", action="store_true",
                      help="Run the opt-in Linux GStreamer candidate library/fixture tests")
    mode.add_argument("--gstreamer-codecs", action="store_true",
                      help="Run the ignored GStreamer/mpv codec comparison with external mpv and FFmpeg")
    mode.add_argument("--prepared-handoff", action="store_true",
                      help="Run the ignored real mpv prepared-source replacement probe")
    mode.add_argument("--gstreamer-handoff", action="store_true",
                      help="Run the ignored mpv/GStreamer prepared-source replacement probe")
    args = parser.parse_args(argv)
    if args.native_controls:
        commands = [
            ["cargo", "test", "-p", "feishin-native-spike", "--test", "control_session",
             "real_mpv_control_routing", "--locked", "--", "--ignored", "--exact"],
            ["cargo", "test", "-p", "feishin-native-spike", "--test", "handoff",
             "real_repeated_handoff_paused_readiness_seek_and_retired_controls",
             "--locked", "--", "--ignored", "--exact"],
            ["cargo", "test", "-p", "feishin-native-spike", "--features", "mpris-probe",
             "--test", "mpris", "real_private_bus_mpv", "--locked", "--", "--ignored", "--exact"],
        ]
    elif args.gstreamer_probe:
        commands = [["cargo", "test", "-p", "feishin-native-spike", "--features", "gst-probe",
                     "--lib", "--test", "gst_probe", "--locked"]]
    elif args.gstreamer_codecs:
        commands = [["cargo", "test", "-p", "feishin-native-spike", "--features", "gst-probe",
                     "--test", "gst_probe", "real_gst_and_mpv_authenticated_codec_comparison",
                     "--locked", "--", "--ignored", "--exact"]]
    elif args.prepared_handoff:
        commands = [["cargo", "test", "-p", "feishin-native-spike", "--test", "prepared_handoff",
                     "real_mpv_prepared_replacement_restore", "--locked", "--", "--ignored", "--exact"]]
    elif args.gstreamer_handoff:
        commands = [["cargo", "test", "-p", "feishin-native-spike", "--features", "gst-probe",
                     "--test", "prepared_handoff", "real_mpv_gstreamer_prepared_replacement_restore",
                     "--locked", "--", "--ignored", "--exact"]]
    elif args.external_audio:
        commands = [["cargo", "test", "-p", "feishin-native-spike", "--test", "native",
                     "--locked", "--no-fail-fast", "--", "--ignored"]]
    else:
        commands = [["cargo", "test", "--workspace", "--locked", "--no-fail-fast"]]
    for command in commands:
        result = run(command)
        if result:
            return result
    return 0


if __name__ == "__main__":
    sys.exit(main())
