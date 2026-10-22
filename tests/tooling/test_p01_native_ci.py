import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("p01_native_ci", ROOT / "tools/app/test-native.py")
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)


class NativeCiTests(unittest.TestCase):
    def test_failed_rust_identifiers_are_reported_without_assertion_payload(self):
        message = ci.annotation("test keyboard_tests::keyboard_only_entry ... FAILED\n")
        self.assertIn("keyboard_tests::keyboard_only_entry failed", message)
        self.assertTrue(message.startswith("::error "))
        for line in ("test success ... ok", "thread panicked: private-token-payload", "test id%0A::warning ... FAILED", "error: test failed", "test failure ... FAILED with private payload"):
            self.assertIsNone(ci.annotation(line))
