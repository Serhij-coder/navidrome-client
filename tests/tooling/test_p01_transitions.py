"""Saved native transition evidence integrity; live capture is explicitly opt-in."""
import hashlib
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class NativeTransitionEvidenceTests(unittest.TestCase):
    def test_all_probe_configurations_and_positive_control_are_preserved(self):
        evidence = ROOT / "docs/app/p01/evidence"
        manifest = json.loads((evidence / "native-transition-manifest.json").read_text())
        for name, digest in manifest["evidence_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest)
        data = json.loads((evidence / "live-native-transitions.json").read_text())
        self.assertEqual(data["sample_rate_hz"], 48000)
        self.assertEqual(data["window_ms"], 1)
        self.assertAlmostEqual(data["quiet_rms_threshold"], 0.02)
        self.assertEqual(data["tone_seconds_each"], 2)
        self.assertEqual(len(data["cases"]), 19)
        ordinary = [case for case in data["cases"] if not case["pause_control"]]
        self.assertEqual(len(ordinary), 18)
        self.assertEqual({(case["format"], case["prefetch_playlist"], case["trial"]) for case in ordinary},
            {(format, prefetch, trial) for format in ("wav", "flac", "mp3") for prefetch in (False, True) for trial in range(3)})
        for case in data["cases"]:
            self.assertFalse(case["load_scripts"])
            self.assertFalse(case["default_prefetch_playlist"])
            self.assertEqual(case["playlist_occurrences_seen"], [True, True])
            self.assertGreater(case["captured_frames"], 48000 * 4)
            self.assertLess(case["captured_frames"], 48000 * 12)
            self.assertLessEqual(case["active_windows_ms"], case["audio_span_ms"])
            self.assertGreaterEqual(case["longest_quiet_run_ms"], 0)
        controls = [case for case in data["cases"] if case["pause_control"]]
        self.assertEqual(len(controls), 1)
        self.assertEqual(controls[0]["format"], "wav")
        self.assertGreaterEqual(controls[0]["longest_quiet_run_ms"], 100)
        self.assertEqual(manifest["checks"]["explicit_transition_captures"], 19)
        self.assertEqual(manifest["checks"]["explicit_existing_native_passed"], 5)
