import importlib.util
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import sys
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("p01_orca", Path(__file__).resolve().parents[1] / "desktop/orca.py")
orca = importlib.util.module_from_spec(spec)
spec.loader.exec_module(orca)


class OrcaTests(unittest.TestCase):
    def test_track_action_mode_requires_consent_before_starting_services(self):
        private = {"P01_OWNED_INPUT_DISPLAY": "1", "DISPLAY": ":71",
                   "DBUS_SESSION_BUS_ADDRESS": "unix:path=/owned/disposable"}
        invalid = [{**private, name: ""} for name in private]
        invalid.extend([{**private, "DISPLAY": "remote:1"},
                        {**private, "DBUS_SESSION_BUS_ADDRESS": "tcp:host=private"}])
        for environment in invalid:
            with self.subTest(environment=environment), \
                 mock.patch.dict(orca.os.environ, environment, clear=True), \
                 mock.patch.object(orca, "child_reaper") as reaper:
                with self.assertRaises(RuntimeError):
                    orca.run(Path("/private-binary"), track_actions=True)
            reaper.assert_not_called()
        with mock.patch.dict(orca.os.environ, private, clear=True), \
             mock.patch.object(orca, "child_reaper"), \
             mock.patch.object(orca, "run_owned", return_value={"owned": True}) as run:
            self.assertEqual(orca.run(Path("/synthetic-binary"), track_actions=True), {"owned": True})
        run.assert_called_once_with(Path("/synthetic-binary"), True)

    def test_saved_before_after_evidence_and_artifacts_match_the_manifest(self):
        evidence = Path(__file__).resolve().parents[2] / "docs/app/p01/evidence"
        manifest = json.loads((evidence / "orca-manifest.json").read_text())
        for name, digest in manifest["evidence_sha256"].items():
            self.assertEqual(hashlib.sha256((evidence / name).read_bytes()).hexdigest(), digest)
        before = json.loads((evidence / "orca-before.json").read_text())
        after = json.loads((evidence / "orca-status.json").read_text())
        self.assertEqual(before["binary_sha256"], manifest["artifacts"]["before"]["sha256"])
        self.assertEqual(after["binary_sha256"], manifest["artifacts"]["status"]["sha256"])
        self.assertNotEqual(before["binary_sha256"], after["binary_sha256"])
        for name in ("filter_name", "scale_name", "table_entry", "exit", "return"):
            self.assertTrue(before["checks"][name])
        for name in ("first", "range", "end", "home"):
            self.assertFalse(before["checks"][name])
        self.assertEqual(before["conclusion"], "failure")
        self.assertEqual(after["conclusion"], "success")
        self.assertEqual(len(after["checks"]), 9)
        self.assertTrue(all(after["checks"].values()))
        self.assertLessEqual(set(after["utterances"]), orca.PHRASES)

    def test_services_and_profiles_do_not_reuse_inherited_session_addresses(self):
        root = Path("/owned")
        env = orca.private_environment(root, {"SPEECHD_ADDRESS": "fixture-unrelated-service",
            "SPEECHD_HOST": "fixture-host", "SPEECHD_PORT": "1", "PULSE_SERVER": "fixture-server",
            "XDG_CONFIG_HOME": "/fixture-profile", "WAYLAND_DISPLAY": "fixture-wayland", "DISPLAY": ":99"})
        self.assertEqual(env["SPEECHD_ADDRESS"], "")
        self.assertEqual(env["SPEECHD_SOCKET"], "/owned/speech.sock")
        self.assertEqual(env["PULSE_SERVER"], "unix:/owned/pulse.sock")
        self.assertEqual(env["XDG_CONFIG_HOME"], "/owned/config")
        self.assertNotIn("WAYLAND_DISPLAY", env)
        self.assertEqual(env["DISPLAY"], ":99")

    def test_only_exact_synthetic_announcements_enter_evidence(self):
        lines = [f"01:02:03.1234 - SPEECH OUTPUT: '{orca.FIRST}' {{voice}}",
                 "01:02:03.1234 - SPEECH OUTPUT: 'private fixture payload' {voice}",
                 f"01:02:03.1234 - SPEECH OUTPUT: '{orca.FIRST} extra payload' {{voice}}",
                 f"untrusted prefix SPEECH OUTPUT: '{orca.FIRST}' {{voice}}"]
        self.assertEqual(orca.utterances("\n".join(lines)), [orca.FIRST])

    def test_log_limit_and_offset_are_enforced(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "log"
            self.assertEqual(orca.read_log(path), "")
            path.write_bytes(b"old\nnew\n")
            self.assertEqual(orca.read_log(path, 4), "new\n")
            with mock.patch.object(orca, "LOG_LIMIT", 4):
                with self.assertRaises(RuntimeError):
                    orca.read_log(path)

    def test_stalled_owned_process_is_killed_and_reaped(self):
        process = mock.Mock(pid=42)
        process.wait.side_effect = [subprocess.TimeoutExpired("fixture", 3), 0]
        with mock.patch.object(orca.os, "killpg", side_effect=[None, None, ProcessLookupError]) as kill, \
             mock.patch.object(orca.os, "waitpid", side_effect=ChildProcessError):
            orca.stop(process)
        self.assertEqual(kill.call_args_list, [mock.call(42, signal.SIGTERM), mock.call(42, signal.SIGKILL), mock.call(42, 0)])
        self.assertEqual(process.wait.call_count, 2)

    def test_cleanup_attempts_every_owned_group_before_reporting_failure(self):
        first, second = mock.Mock(), mock.Mock()
        with mock.patch.object(orca, "stop", side_effect=[subprocess.TimeoutExpired("fixture", 3), None]) as stop:
            with self.assertRaises(RuntimeError):
                orca.cleanup([first, second])
        self.assertEqual(stop.call_args_list, [mock.call(second), mock.call(first)])

    @unittest.skipUnless(sys.platform == "linux", "Linux-only Orca harness")
    def test_exited_leader_does_not_leave_term_ignoring_descendant(self):
        # Run a real session leader and stubborn grandchild. child_reaper lets
        # this test verify the grandchild is killed AND reaped, not a zombie.
        peer = ("import os,signal,sys,time;"
                "pid=os.fork();"
                "signal.signal(signal.SIGTERM,signal.SIG_IGN) if pid==0 else None;"
                "open(sys.argv[1],'w').write(str(os.getpid())) if pid==0 else None;"
                "time.sleep(60)")
        with tempfile.TemporaryDirectory() as temporary, orca.child_reaper():
            ready = Path(temporary) / "child"
            process = subprocess.Popen([sys.executable, "-c", peer, str(ready)], start_new_session=True)
            try:
                deadline = time.monotonic() + 3
                while not ready.exists() or not ready.read_text():
                    if time.monotonic() >= deadline:
                        self.fail("owned descendant did not become ready")
                    time.sleep(0.01)
                child = int(ready.read_text())
                os.kill(child, 0)
                orca.stop(process)
                self.assertEqual(process.returncode, -signal.SIGTERM)
                with self.assertRaises(ProcessLookupError):
                    os.kill(child, 0)
                with self.assertRaises(ChildProcessError):
                    os.waitpid(child, os.WNOHANG)
                with self.assertRaises(ProcessLookupError):
                    os.killpg(process.pid, 0)
            finally:
                # Independent teardown must also work if stop() regresses.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=3)
                deadline = time.monotonic() + 3
                while True:
                    try:
                        if os.waitpid(-process.pid, os.WNOHANG)[0]:
                            continue
                    except ChildProcessError:
                        break
                    if time.monotonic() >= deadline:
                        self.fail("fixture descendant could not be reaped")
                    time.sleep(0.01)
