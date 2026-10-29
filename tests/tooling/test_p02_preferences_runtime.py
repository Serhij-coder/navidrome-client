import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "desktop/preferences_runtime.py"


class PreferenceRuntimeIsolationTests(unittest.TestCase):
    def test_refuses_non_owned_displays_before_loading_gui_or_opening_profile(self):
        spec = importlib.util.spec_from_file_location("preference_runtime", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        for environment in (
            {},
            {"DISPLAY": ":99", "DBUS_SESSION_BUS_ADDRESS": "unix:path=/fixture"},
            {"P02_OWNED_INPUT_DISPLAY": "1", "DISPLAY": "remote:0", "DBUS_SESSION_BUS_ADDRESS": "unix:path=/fixture"},
            {"P02_OWNED_INPUT_DISPLAY": "1", "DISPLAY": ":99"},
        ):
            with self.subTest(environment=environment), patch.dict(os.environ, environment, clear=True):
                with self.assertRaisesRegex(RuntimeError, "explicit owned"):
                    module.run(Path("/nonexistent/unused-binary"))

    def test_refuses_to_overwrite_existing_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "evidence.json"
            output.write_bytes(b"original evidence")
            result = subprocess.run([sys.executable, str(SCRIPT), "/unused", str(output)],
                                    capture_output=True, text=True, timeout=5)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("refusing to overwrite", result.stderr)
            self.assertEqual(output.read_bytes(), b"original evidence")
