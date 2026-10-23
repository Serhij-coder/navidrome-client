"""Pinned source replay and native crossfade evidence; no audio in default tests."""
import json
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
REFERENCE = os.environ.get("P00_REFERENCE_ROOT")
PARSER = os.environ.get("P00_TYPESCRIPT")


class CrossfadeEvidenceTests(unittest.TestCase):
    def test_saved_source_quirks_and_volume_mapping_are_explicit(self):
        data = json.loads((ROOT / "tests/fixtures/p01/crossfade/upstream-gains.json").read_text())
        self.assertEqual(data["baseline"], "7d6ad318c79802e226996cfc39acba6b05bb22e8")
        self.assertEqual(data["parser"], "6.0.3")
        self.assertEqual(len(data["functions"]), 10)
        self.assertEqual(len(data["samples"]), 198)
        def point(style, progress, master=1):
            return next(s for s in data["samples"] if s["style"] == style and s["progress"] == progress and s["master_fraction"] == master)
        self.assertEqual(point("exponential", 0)["outgoing"], 0)
        self.assertEqual(point("exponential", 1)["outgoing"], 1)
        self.assertGreater(point("exponential", 1)["incoming"], 0.99)
        self.assertAlmostEqual(point("equalPower", 0.5)["web_outgoing_gain"], 0.5)
        self.assertAlmostEqual(point("equalPower", 0.5, 0.5)["web_outgoing_gain"], 0.125)
        for style in ("dipped", "constantPower", "constantPowerSlowCut", "constantPowerSlowFade", "unknown-saved-style"):
            self.assertAlmostEqual(point(style, 0.5)["outgoing"], point("equalPower", 0.5)["outgoing"])

    def test_modified_source_is_rejected_without_execution_or_payload_logging(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "src/renderer/features/player/audio-player/web-player.tsx"
            source.parent.mkdir(parents=True)
            source.write_text("throw new Error('private-source-fixture');")
            result = subprocess.run(["node", str(ROOT / "tests/reference/p01-crossfade.cjs"), str(root), "unused-parser", str(root / "result.json")], capture_output=True, text=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("private-source-fixture", result.stdout + result.stderr)
            self.assertFalse((root / "result.json").exists())

    @unittest.skipUnless(REFERENCE and PARSER, "set frozen P00_REFERENCE_ROOT/P00_TYPESCRIPT for optional source replay")
    def test_original_pinned_replay_matches_saved_values(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "result.json"
            result = subprocess.run(["node", str(ROOT / "tests/reference/p01-crossfade.cjs"), REFERENCE, PARSER, str(output)], capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            expected = json.loads((ROOT / "tests/fixtures/p01/crossfade/upstream-gains.json").read_text())
            self.assertEqual(json.loads(output.read_text()), expected)

    def test_live_evidence_preserves_both_tones_controls_and_provenance(self):
        evidence = ROOT / "docs/app/p01/evidence"
        manifest = json.loads((evidence / "crossfade-manifest.json").read_text())
        for name, digest in manifest["evidence_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest)
        calibration = json.loads((evidence / "native-volume-calibration.json").read_text())
        self.assertEqual([c["percent"] for c in calibration["cases"]], [100, 50, 25, 0, 100])
        base = calibration["cases"][0]["amplitude"]
        self.assertAlmostEqual(calibration["cases"][1]["amplitude"] / base, 0.125, delta=0.005)
        self.assertAlmostEqual(calibration["cases"][2]["amplitude"] / base, 0.015625, delta=0.002)
        self.assertLess(calibration["cases"][3]["amplitude"], 0.002)
        native = json.loads((evidence / "live-native-crossfade.json").read_text())
        self.assertEqual([c["curve"] for c in native["cases"]], ["Linear", "EqualPower", "SCurve"])
        for case in native["cases"]:
            self.assertEqual(case["fade_media_seconds"], 3)
            self.assertEqual(case["speed"], 1)
            self.assertLess(case["captured_frames"], 48000 * 24)
            self.assertLess(case["outgoing_tail_relative"], 0.02)
            self.assertLess(case["incoming_head_relative"], 0.02)
            self.assertTrue(0.05 < case["outgoing_mid_relative"] < 0.85)
            self.assertTrue(0.05 < case["incoming_mid_relative"] < 0.85)
            self.assertLess(case["peak"], 0.95)
            self.assertEqual(case["commands"][-1]["media_progress"], 1)
            self.assertTrue(case["spectral_windows"])
        regression = json.loads((evidence / "crossfade-transition-regression.json").read_text())
        self.assertEqual(len(regression["cases"]), 19)
