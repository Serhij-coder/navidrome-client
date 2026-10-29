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
    CONTROL_COMMANDS = [
        ["cargo", "test", "-p", "feishin-native-spike", "--test", "control_session",
         "real_mpv_control_routing", "--locked", "--", "--ignored", "--exact"],
        ["cargo", "test", "-p", "feishin-native-spike", "--test", "handoff",
         "real_repeated_handoff_paused_readiness_seek_and_retired_controls",
         "--locked", "--", "--ignored", "--exact"],
        ["cargo", "test", "-p", "feishin-native-spike", "--features", "mpris-probe",
         "--test", "mpris", "real_private_bus_mpv", "--locked", "--", "--ignored", "--exact"],
    ]

    def test_failed_rust_identifiers_are_reported_without_assertion_payload(self):
        message = ci.annotation("test keyboard_tests::keyboard_only_entry ... FAILED\n")
        self.assertIn("keyboard_tests::keyboard_only_entry failed", message)
        self.assertTrue(message.startswith("::error "))
        for line in ("test success ... ok", "thread panicked: private-token-payload", "test id%0A::warning ... FAILED", "error: test failed", "test failure ... FAILED with private payload"):
            self.assertIsNone(ci.annotation(line))

    def test_allowlisted_panic_locations_exclude_thread_and_payload(self):
        for prefix in ("", "crates/native-spike/../../", "/home/runner/work/repo/repo/"):
            for filename in ("mpris", "handoff", "control_session", "prepared_handoff"):
                with self.subTest(prefix=prefix, filename=filename):
                    line = (f"thread 'private-marker' (123) panicked at "
                            f"{prefix}tests/native-spike/{filename}.rs:246:18:\n")
                    result = ci.location_annotation(line)
                    self.assertTrue(result.startswith(
                        f"::error file=tests/native-spike/{filename}.rs,line=246,col=18,"))
                    self.assertNotIn("private-marker", result)
                    self.assertNotIn("runner", result)
        self.assertIsNotNone(ci.location_annotation(
            "thread 'fixture' panicked at tests/native-spike/mpris.rs:1:1:"))
        for line in (
            "thread 'x' panicked at tests/native-spike/unlisted.rs:1:1:",
            "thread 'x' panicked at tests/storage/mpris.rs:1:1:",
            "thread 'x' panicked at https://private/tests/native-spike/mpris.rs:1:1:",
            "thread 'x' panicked at tests/native-spike/mpris.rs:0:1:",
            "thread 'x' panicked at tests/native-spike/mpris.rs:1:0:",
            "thread 'x' panicked at tests/native-spike/mpris.rs:1000000:1:",
            "thread 'x' panicked at tests/native-spike/mpris.rs:1:1: private-marker",
            "thread 'x' panicked at tests/native-spike/mpris.rs:1:1:%0A::warning",
            "thread 'x\nprivate-marker' panicked at tests/native-spike/mpris.rs:1:1:",
        ):
            self.assertIsNone(ci.location_annotation(line), line)

    def test_location_annotation_streaming_preserves_exit_and_ci_scope(self):
        header = ("thread 'private-marker' (4) panicked at "
                  "crates/native-spike/../../tests/native-spike/mpris.rs:246:18:\n")
        for enabled in ("true", "false"):
            process = mock.Mock(stdout=iter([header, "private assertion payload\n"]))
            process.wait.return_value = 101
            output = io.StringIO()
            with mock.patch.object(ci.subprocess, "Popen", return_value=process), \
                 mock.patch.dict(ci.os.environ, {"GITHUB_ACTIONS": enabled}), \
                 contextlib.redirect_stdout(output):
                self.assertEqual(ci.run(["cargo", "test"]), 101)
            annotations = [line for line in output.getvalue().splitlines() if line.startswith("::")]
            self.assertEqual(annotations, [ci.location_annotation(header)] if enabled == "true" else [])
            self.assertIn(header, output.getvalue())
            self.assertTrue(all("private" not in line for line in annotations))

    def test_modes_keep_the_exact_test_scope_and_preserve_cargo_failure(self):
        for arguments, command in [
            ([], ["cargo", "test", "--workspace", "--locked", "--no-fail-fast"]),
            (["--external-audio"], ["cargo", "test", "-p", "feishin-native-spike", "--test", "native",
                                    "--locked", "--no-fail-fast", "--", "--ignored"]),
            (["--gstreamer-probe"], ["cargo", "test", "-p", "feishin-native-spike", "--features", "gst-probe",
                                      "--lib", "--test", "gst_probe", "--locked"]),
            (["--gstreamer-codecs"], ["cargo", "test", "-p", "feishin-native-spike", "--features", "gst-probe",
                                       "--test", "gst_probe", "real_gst_and_mpv_authenticated_codec_comparison",
                                       "--locked", "--", "--ignored", "--exact"]),
            (["--prepared-handoff"], ["cargo", "test", "-p", "feishin-native-spike", "--test", "prepared_handoff",
                                       "real_mpv_prepared_replacement_restore", "--locked", "--", "--ignored", "--exact"]),
            (["--gstreamer-handoff"], ["cargo", "test", "-p", "feishin-native-spike", "--features", "gst-probe",
                                        "--test", "prepared_handoff", "real_mpv_gstreamer_prepared_replacement_restore",
                                        "--locked", "--", "--ignored", "--exact"]),
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

    def test_native_controls_preserves_exact_order_and_stops_at_each_failure(self):
        for failure_index in [None, 0, 1, 2]:
            with self.subTest(failure_index=failure_index):
                processes = []
                for index in range(3):
                    process = mock.Mock()
                    failed = index == failure_index
                    process.stdout = iter([
                        f"test fixture_{index} ... {'FAILED' if failed else 'ok'}\n",
                        "thread panicked: fixture-only-private-marker\n" if failed else "",
                    ])
                    process.wait.return_value = 101 if failed else 0
                    processes.append(process)
                output = io.StringIO()
                with mock.patch.object(ci.subprocess, "Popen", side_effect=processes) as popen, \
                     mock.patch.dict(ci.os.environ, {"GITHUB_ACTIONS": "true"}), \
                     contextlib.redirect_stdout(output):
                    self.assertEqual(ci.main(["--native-controls"]),
                                     0 if failure_index is None else 101)
                count = 3 if failure_index is None else failure_index + 1
                self.assertEqual(popen.call_args_list, [
                    mock.call(command, stdout=ci.subprocess.PIPE,
                              stderr=ci.subprocess.STDOUT, text=True)
                    for command in self.CONTROL_COMMANDS[:count]
                ])
                for process in processes[:count]:
                    process.wait.assert_called_once_with()
                for process in processes[count:]:
                    process.wait.assert_not_called()
                annotations = [line for line in output.getvalue().splitlines()
                               if line.startswith("::")]
                expected = ([] if failure_index is None else
                            [ci.annotation(f"test fixture_{failure_index} ... FAILED")])
                self.assertEqual(annotations, expected)
                if failure_index is not None:
                    self.assertIn("thread panicked: fixture-only-private-marker\n", output.getvalue())
                    self.assertNotIn("private-marker", annotations[0])

    def test_modes_are_mutually_exclusive_and_reject_options_before_running(self):
        for arguments in (["--native-controls", "--external-audio"],
                          ["--gstreamer-probe", "--gstreamer-codecs"],
                          ["--gstreamer-probe", "--external-audio"],
                          ["--gstreamer-codecs", "--native-controls"],
                          ["--prepared-handoff", "--native-controls"],
                          ["--gstreamer-handoff", "--gstreamer-codecs"],
                          ["--prepared-handoff", "--gstreamer-handoff"],
                          ["--gstreamer-handoff", "--unrecognized"],
                          ["--gstreamer-probe", "--unrecognized"],
                          ["--native-controls", "--unrecognized"]):
            with self.subTest(arguments=arguments), \
                 mock.patch.object(ci.subprocess, "Popen") as popen, \
                 contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    ci.main(arguments)
                self.assertEqual(raised.exception.code, 2)
                popen.assert_not_called()
