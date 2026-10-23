"""Regression for failure recording and saved evidence; real browser media runs are separate."""
import json
import hashlib
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

    def test_saved_seek_comparison_preserves_failures_and_manifest_integrity(self):
        manifest = json.loads((ROOT / "docs/app/p01/evidence/browser-seeking.json").read_text())
        for name, digest in manifest["evidence_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest)
        data = json.loads((ROOT / "tests/fixtures/p01/browser/2026-10-23-seeking/browser-results.json").read_text())
        self.assertEqual(data["playwright"], "1.56.1")
        browsers = {item["browser"]: item for item in data["results"]}
        self.assertEqual(set(browsers), {"chromium", "firefox", "webkit"})
        for item in browsers.values():
            self.assertEqual(item["shell"], "passed")
            self.assertGreater(item["analysisPeak"], 0.01)
            self.assertFalse(item["sessionSeek"]["loop"])
            self.assertEqual(item["sessionSeek"]["completion"], "seeked")
            self.assertEqual(item["sessionSeek"]["observed"], 2)
            self.assertEqual(item["serverStreams"], [])
            self.assertEqual([case["loop"] for case in item["loopSeekComparison"]], [False, True])
            self.assertTrue(all([seek["target"] for seek in case["seeks"]] == [2, 5] for case in item["loopSeekComparison"]))
        self.assertEqual(browsers["chromium"]["seekingGate"], "passed")
        self.assertTrue(all(edge["result"] == "passed" for edge in browsers["chromium"]["sessionSeek"]["edges"]))
        firefox = browsers["firefox"]
        self.assertEqual(firefox["seekingGate"], "open")
        self.assertEqual(firefox["sessionSeek"]["edges"][-1]["mediaError"], 3)
        for case in firefox["loopSeekComparison"]:
            self.assertEqual(case["seeks"][0]["result"], "passed")
            self.assertEqual(case["seeks"][1]["mediaError"], 3)
            self.assertEqual(case["seeks"][1]["completion"], "error")
        webkit = browsers["webkit"]
        self.assertEqual(webkit["seekingGate"], "open")
        self.assertEqual(webkit["sessionSeek"]["edges"][-1]["completion"], "timeout")
        self.assertEqual(webkit["sessionSeek"]["edges"][-1]["observed"], 0)
        self.assertTrue(all(seek["result"] == "passed" for seek in webkit["loopSeekComparison"][0]["seeks"]))
        self.assertTrue(all(seek["observed"] == 0 and seek["result"] == "lab-seek-failed" for seek in webkit["loopSeekComparison"][1]["seeks"]))

    def test_saved_seek_deadlines_cover_failure_recovery_and_cancellation(self):
        manifest = json.loads((ROOT / "docs/app/p01/evidence/seek-deadlines.json").read_text())
        for name, digest in manifest["evidence_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest)
        evidence = ROOT / "tests/fixtures/p01/browser/2026-10-23-seek-deadline"
        before = json.loads((evidence / "before.json").read_text())
        after = json.loads((evidence / "fixed.json").read_text())
        self.assertEqual([b["browser"] for b in before["results"]], ["chromium", "firefox", "webkit"])
        self.assertTrue(all(b["stage"] == "seek-lifecycle" and b["result"] == "failed-or-unavailable" for b in before["results"]))
        self.assertEqual([b["browser"] for b in after["results"]], ["chromium", "firefox", "webkit"])
        cases = {"setterRejection", "samePosition", "timeout", "replacement", "sourceChange", "recovery", "failedResume", "nativeResume", "shutdown"}
        for browser in after["results"]:
            self.assertEqual(browser["shell"], "passed")
            self.assertEqual(set(browser["seekLifecycleChecks"]), cases)
            self.assertTrue(all(result == "passed" for result in browser["seekLifecycleChecks"].values()))
            self.assertEqual(browser["pageErrors"], [])
            self.assertEqual(browser["serverStreams"], [])
            self.assertGreater(browser["analysisPeak"], 0.01)
        for browser in after["results"][1:]:
            self.assertEqual(browser["seekingGate"], "open")
            self.assertEqual(browser["sessionPlay"], "visible-reload-required")
        native_before = json.loads((evidence / "native-resume-before.json").read_text())
        self.assertEqual([b["browser"] for b in native_before["results"]], ["chromium", "firefox", "webkit"])
        self.assertTrue(all(b["stage"] == "seek-lifecycle" and b["failureType"] == "NativeResumeFailure" for b in native_before["results"]))
        self.assertNotIn("private-disposable-seek-error", (evidence / "fixed.json").read_text())
