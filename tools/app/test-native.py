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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--external-audio", action="store_true",
                        help="Run ignored native probes with an explicitly provisioned owned PulseAudio sink")
    args = parser.parse_args(argv)
    command = (["cargo", "test", "-p", "feishin-native-spike", "--test", "native",
                "--locked", "--no-fail-fast", "--", "--ignored"] if args.external_audio else
               ["cargo", "test", "--workspace", "--locked", "--no-fail-fast"])
    process = subprocess.Popen(command,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    assert process.stdout is not None  # PIPE invariant, not external input.
    for line in process.stdout:
        print(line, end="", flush=True)
        if os.environ.get("GITHUB_ACTIONS") == "true" and (message := annotation(line)):
            print(message, flush=True)
    return process.wait()


if __name__ == "__main__":
    sys.exit(main())
