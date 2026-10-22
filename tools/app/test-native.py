#!/usr/bin/env python3
"""Run tracked native tests and expose failed test names as CI annotations.

Public check annotations remain readable when detailed hosted logs need login.
Only Rust test identifiers enter annotations; assertion data/payloads do not.
"""
import os
import re
import subprocess
import sys

FAILED = re.compile(r"^test ([A-Za-z0-9_:]+) \.\.\. FAILED$")


def annotation(line):
    match = FAILED.fullmatch(line.strip())
    return f"::error title=Native Rust regression::{match[1]} failed; inspect the authenticated job log for assertion details" if match else None


def main():
    process = subprocess.Popen(["cargo", "test", "--workspace", "--locked", "--no-fail-fast"],
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    assert process.stdout is not None  # PIPE invariant, not external input.
    for line in process.stdout:
        print(line, end="", flush=True)
        if os.environ.get("GITHUB_ACTIONS") == "true" and (message := annotation(line)):
            print(message, flush=True)
    return process.wait()


if __name__ == "__main__":
    sys.exit(main())
