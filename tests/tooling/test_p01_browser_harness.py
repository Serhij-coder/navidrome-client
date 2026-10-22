"""Regression for failure recording only; real browser media runs are separate."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

class BrowserFailureTests(unittest.TestCase):
    def test_failed_launch_records_every_browser_without_logging_error_payloads(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stub = root / "playwright.cjs"
            stub.write_text("const engine={launch:async()=>{const e=new Error('private-error-payload');e.name='FixtureLaunchFailure';throw e;}};module.exports={chromium:engine,firefox:engine,webkit:engine};")
            env = {**os.environ, "P01_PLAYWRIGHT": str(stub)}
            result = subprocess.run(["node", str(ROOT / "tests/browser/p01-browser.cjs"), str(root), str(root)], env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(1, result.returncode)
            self.assertNotIn("private-error-payload", result.stdout + result.stderr)
            data = json.loads((root / "browser-results.json").read_text())
            self.assertEqual(["chromium","firefox","webkit"], [r["browser"] for r in data["results"]])
            self.assertTrue(all(r["result"] == "failed-or-unavailable" and r["stage"] == "launch" for r in data["results"]))

    def test_malformed_private_fixture_does_not_echo_json_payload(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stub = root / "playwright.cjs"
            stub.write_text("module.exports={};")
            private = root / "private.json"
            private.write_text('{"credential":"private-payload-to-hide", malformed')
            env = {**os.environ, "P01_PLAYWRIGHT":str(stub), "P01_SERVER_STREAMS":str(private)}
            result = subprocess.run(["node", str(ROOT / "tests/browser/p01-browser.cjs"), str(root), str(root)], env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(1, result.returncode)
            self.assertNotIn("private-payload-to-hide", result.stdout + result.stderr)
            self.assertIn("no contents logged", result.stderr)
