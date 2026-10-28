import contextlib
import importlib.util
import io
from pathlib import Path
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("p01_notice_features", ROOT / "tools/app/notices.py")
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)


class NoticeFeatureTests(unittest.TestCase):
    def test_metadata_uses_explicit_feature_scope_without_changing_default(self):
        for features in [None, "feishin-native-spike/gst-probe"]:
            with self.subTest(features=features):
                arguments = ["notices.py", "/tmp/owned-notice-output"]
                expected = ["cargo", "metadata", "--locked", "--format-version", "1"]
                if features:
                    arguments.extend(["--features", features])
                    expected.extend(["--features", features])
                with mock.patch("sys.argv", arguments), \
                     mock.patch.object(notices.subprocess, "check_output", return_value=b'{"packages": []}') as cargo, \
                     mock.patch.object(notices, "collect", return_value=[]) as collect, \
                     contextlib.redirect_stdout(io.StringIO()):
                    notices.main()
                cargo.assert_called_once_with(expected, cwd=ROOT)
                collect.assert_called_once_with({"packages": []}, Path("/tmp/owned-notice-output"))

    def test_metadata_failure_cannot_become_a_partial_notice_success(self):
        error = notices.subprocess.CalledProcessError(101, ["cargo", "metadata"])
        with mock.patch("sys.argv", ["notices.py", "/tmp/owned-notice-output", "--features", "missing"]), \
             mock.patch.object(notices.subprocess, "check_output", side_effect=error), \
             mock.patch.object(notices, "collect") as collect:
            with self.assertRaises(notices.subprocess.CalledProcessError):
                notices.main()
        collect.assert_not_called()
