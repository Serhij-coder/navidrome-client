import importlib.util
import contextlib
import io
from pathlib import Path
import unittest
from unittest import mock

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

    def test_modes_keep_the_exact_test_scope_and_preserve_cargo_failure(self):
        for arguments, command in [
            ([], ["cargo", "test", "--workspace", "--locked", "--no-fail-fast"]),
            (["--external-audio"], ["cargo", "test", "-p", "feishin-native-spike", "--test", "native",
                                    "--locked", "--no-fail-fast", "--", "--ignored"]),
        ]:
            with self.subTest(arguments=arguments):
                process = mock.Mock()
                process.stdout = iter(["test owned_mpv_monitor_capture_and_teardown ... FAILED\n",
                                       "thread panicked: fixture-only-private-marker\n"])
                process.wait.return_value = 101
                output = io.StringIO()
                with mock.patch.object(ci.subprocess, "Popen", return_value=process) as popen, \
                     mock.patch.dict(ci.os.environ, {"GITHUB_ACTIONS": "true"}), \
                     contextlib.redirect_stdout(output):
                    self.assertEqual(ci.main(arguments), 101)
                popen.assert_called_once_with(command, stdout=ci.subprocess.PIPE,
                                              stderr=ci.subprocess.STDOUT, text=True)
                process.wait.assert_called_once_with()
                annotations = [line for line in output.getvalue().splitlines() if line.startswith("::")]
                self.assertEqual(annotations, [ci.annotation("test owned_mpv_monitor_capture_and_teardown ... FAILED")])
                self.assertNotIn("private-marker", annotations[0])

    def test_success_outside_ci_has_no_annotation_and_unknown_options_do_not_run(self):
        process = mock.Mock(stdout=iter(["test fixture ... ok\n"]))
        process.wait.return_value = 0
        output = io.StringIO()
        with mock.patch.object(ci.subprocess, "Popen", return_value=process), \
             mock.patch.dict(ci.os.environ, {"GITHUB_ACTIONS": "false"}), \
             contextlib.redirect_stdout(output):
            self.assertEqual(ci.main([]), 0)
        self.assertEqual(output.getvalue(), "test fixture ... ok\n")
        with mock.patch.object(ci.subprocess, "Popen") as popen, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as raised:
                ci.main(["--external-audio", "--unrecognized"])
            self.assertEqual(raised.exception.code, 2)
            popen.assert_not_called()
