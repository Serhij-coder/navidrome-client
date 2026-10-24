"""Input harness boundaries; real engine runs are recorded separately."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
HARNESS = ROOT / "tests/browser/input.cjs"


class InputHarnessTests(unittest.TestCase):
    def test_static_server_rejects_escaped_paths(self):
        script = """
const {safeFile,summaryPattern}=require(process.argv[1]);
const assert=require('node:assert/strict');
assert.equal(new RegExp(summaryPattern('0 rows, 0 selected')).test('1000 rows, 0 selected.'),false);
assert.equal(new RegExp(summaryPattern('0 rows, 0 selected')).test('0 rows, 0 selected.'),true);
assert.equal(safeFile('/fixture','/web/'),'/fixture/web/index.html');
assert.equal(safeFile('/fixture','/web/app.js?ignored'),'/fixture/web/app.js');
for(const input of ['/%2e%2e%2fsecret','/%00','/%5csecret','/%zz'])
  assert.equal(safeFile('/fixture',input),null);
"""
        subprocess.run(["node", "-e", script, str(HARNESS)], check=True, timeout=10)

    def test_launch_failure_records_all_journeys_and_hides_payload(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for shell in ("web", "remote"):
                (root / shell).mkdir()
                for name in ("index.html", "app.js", "app_bg.wasm"):
                    (root / shell / name).write_text("synthetic")
            stub = root / "playwright.cjs"
            stub.write_text("const engine={launch:async()=>{throw new TypeError('private-error-payload')}};module.exports={chromium:engine,webkit:engine,firefox:engine}")
            output = root / "output"
            env = {**os.environ, "P01_PLAYWRIGHT": str(stub)}
            run = subprocess.run(["node", str(HARNESS), str(root), str(output)], env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(run.returncode, 1)
            self.assertNotIn("private-error-payload", run.stdout + run.stderr)
            data = json.loads((output / "results.json").read_text())
            self.assertEqual(len(data["results"]), 18)
            self.assertTrue(all(r["result"] == "failed" and r["stage"] == "launch" and r["failureType"] == "TypeError" for r in data["results"]))
            self.assertEqual(len(data["bundles"]), 6)
            # Existing evidence must never be overwritten.
            saved = (output / "results.json").read_bytes()
            subprocess.run(["node", str(HARNESS), str(root), str(output)], env=env, capture_output=True, timeout=10)
            self.assertEqual(saved, (output / "results.json").read_bytes())

    def test_cleanup_attempts_browser_after_context_close_rejection(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for shell in ("web", "remote"):
                (root / shell).mkdir()
                for name in ("index.html", "app.js", "app_bg.wasm"):
                    (root / shell / name).write_text("synthetic")
            marker = root / "closed"
            stub = root / "playwright.cjs"
            stub.write_text("const fs=require('fs');const engine={launch:async()=>({version:()=> 'fixture',newContext:async()=>({newPage:async()=>{throw new Error('private-startup')},close:async()=>{throw new Error('private-cleanup')}}),close:async()=>fs.appendFileSync(process.env.CLOSE_MARKER,'x')})};module.exports={chromium:engine,webkit:engine,firefox:engine}")
            env = {**os.environ, "P01_PLAYWRIGHT": str(stub), "CLOSE_MARKER": str(marker)}
            run = subprocess.run(["node", str(HARNESS), str(root), str(root / "output")], env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(run.returncode, 1)
            self.assertEqual(marker.read_text(), "x" * 18)
            data = json.loads((root / "output/results.json").read_text())
            self.assertTrue(all(r["cleanupFailure"] for r in data["results"]))
            self.assertNotIn("private-startup", run.stdout + run.stderr)
            self.assertNotIn("private-cleanup", run.stdout + run.stderr)


if __name__ == "__main__":
    unittest.main()
