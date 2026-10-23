import importlib.util
import json
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tests/desktop/track_actions.py"
spec = importlib.util.spec_from_file_location("p01_track_actions", SCRIPT)
journey = importlib.util.module_from_spec(spec)
spec.loader.exec_module(journey)


class TrackActionJourneyTests(unittest.TestCase):
    def test_private_display_and_explicit_consent_are_required(self):
        private = {"P01_OWNED_INPUT_DISPLAY": "1", "DISPLAY": ":71", "DBUS_SESSION_BUS_ADDRESS": "unix:path=/owned/disposable"}
        self.assertTrue(journey.owned_display(private))
        for name in private:
            omitted = private.copy()
            omitted.pop(name)
            self.assertFalse(journey.owned_display(omitted))
        self.assertFalse(journey.owned_display({**private, "DISPLAY": "remote:1"}))
        self.assertFalse(journey.owned_display({**private, "DBUS_SESSION_BUS_ADDRESS": "tcp:host=private"}))

    def test_missing_consent_fails_before_gui_import_or_private_payload_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "evidence.json"
            environment = {key: value for key, value in os.environ.items() if key != "P01_OWNED_INPUT_DISPLAY"}
            result = subprocess.run(["python3", str(SCRIPT), "/private-binary-marker", str(output)], env=environment,
                                    capture_output=True, text=True, timeout=5)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("private-binary-marker", result.stdout + result.stderr)
            self.assertFalse(output.exists())

    def test_saved_native_menu_and_clipboard_evidence(self):
        evidence = ROOT / "docs/app/p01/evidence"
        manifest = json.loads((evidence / "track-actions-manifest.json").read_text())
        for name, expected in manifest["evidence_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), expected)
        before = json.loads((evidence / "track-actions-before.json").read_text())
        fixed = json.loads((evidence / "track-actions-fixed.json").read_text())
        self.assertEqual(before["checks"], {"table_entry": True, "first_track": True, "keyboard_menu": False})
        self.assertEqual(before["conclusion"], "failure")
        self.assertEqual(fixed["conclusion"], "success")
        self.assertEqual(len(fixed["checks"]), 15)
        self.assertTrue(all(fixed["checks"].values()))
        self.assertNotEqual(before["binary_sha256"], fixed["binary_sha256"])
        intermediate = json.loads((evidence / "track-actions-orca-before.json").read_text())
        spoken = json.loads((evidence / "track-actions-orca-fixed.json").read_text())
        self.assertTrue(all(value for name, value in intermediate["checks"].items() if not name.startswith("actions_")))
        self.assertFalse(intermediate["checks"]["actions_reopen"])
        self.assertFalse(intermediate["checks"]["actions_copy_return"])
        self.assertEqual(spoken["conclusion"], "success")
        self.assertEqual(len(spoken["checks"]), 19)
        self.assertTrue(all(spoken["checks"].values()))
        self.assertIn("Copy synthetic title push button.", spoken["utterances"])
        self.assertEqual(spoken["binary_sha256"], fixed["binary_sha256"])
