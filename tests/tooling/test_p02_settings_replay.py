"""The optional replay must reject altered source before loading external code."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class SettingsReplayGuard(unittest.TestCase):
    @unittest.skipUnless(shutil.which("node"), "Node is required for source replay guard")
    def test_modified_source_is_rejected_before_compiler_or_fixture_write(self):
        fixture = ROOT / "tests/fixtures/p02/settings-migrations/source-replay.json"
        early = ROOT / "tests/fixtures/p02/settings-migrations/early-source-replay.json"
        original = {fixture: fixture.read_bytes(), early: early.read_bytes()}
        with tempfile.TemporaryDirectory() as temporary:
            copied = Path(temporary) / "src/renderer/store/settings.store.ts"
            copied.parent.mkdir(parents=True)
            canary = "UNTRUSTED_REPLAY_SOURCE_CANARY"
            copied.write_text(canary, encoding="utf-8")
            for flags in [["--write"], ["--early", "--write"]]:
                with self.subTest(flags=flags):
                    result = subprocess.run(
                        ["node", str(ROOT / "tests/reference/p02-settings-migrations.cjs"),
                         temporary, str(Path(temporary) / "missing-compiler.js"),
                         str(Path(temporary) / "missing-lodash.js"), *flags],
                        text=True, capture_output=True, timeout=15, check=False,
                    )
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("Pinned source mismatch", result.stderr)
                    self.assertNotIn("Cannot find module", result.stderr)
                    self.assertNotIn(canary, result.stdout + result.stderr)
                    for path, expected in original.items():
                        self.assertEqual(path.read_bytes(), expected)
