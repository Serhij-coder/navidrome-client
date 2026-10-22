import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/fixtures/p00"
SPEC = importlib.util.spec_from_file_location("p00_capture", ROOT / "tests/reference/p00-live-capture.py")
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


class RuntimeEvidence(unittest.TestCase):
    def test_font_license_snapshots_and_archive_hash_cross_reference(self):
        import csv
        baseline = ROOT / "docs/app/baseline"
        report = json.loads((baseline / "font-license-evidence.json").read_text())
        with (baseline / "assets.csv").open() as handle:
            assets = {row["upstream_path"]: row["sha256"] for row in csv.DictReader(handle)}
        self.assertEqual(12, len(report["fonts"]))
        for font in report["fonts"]:
            self.assertEqual(assets[font["path"]], font["sha256"])
            self.assertTrue(font["embedded_names"]["5"])
        for license in report["publisher_licenses"]:
            data = json.loads((baseline / license["path"]).read_text())["text"].encode("utf-8")
            self.assertEqual(license["sha256"], hashlib.sha256(data).hexdigest())
            self.assertIn(b"SIL OPEN FONT LICENSE Version 1.1", data)

    def test_live_extension_and_permission_differences(self):
        report = json.loads((FIXTURES / "live-responses.json").read_text())
        self.assertEqual(6, len(report["summaries"]))
        self.assertTrue(all(row["readerAuthentication"] == "ok" for row in report["summaries"]))
        records = report["records"]
        find = lambda fixture, path: next(r for r in records if r["fixture"] == fixture and r["path"] == path)
        self.assertEqual(404, find("navidrome-0.47.5:admin", "/rest/getOpenSubsonicExtensions")["http_status"])
        self.assertEqual(70, find("gonic-0.15.2:admin", "/rest/getOpenSubsonicExtensions")["body"]["subsonic-response"]["error"]["code"])
        self.assertEqual(50, find("navidrome-0.61.0:reader", "/rest/startScan")["body"]["subsonic-response"]["error"]["code"])
        self.assertEqual("ok", find("gonic-0.16.4:reader", "/rest/startScan")["body"]["subsonic-response"]["status"])
        for version in ["0.47.5", "0.61.0"]:
            self.assertFalse(find(f"navidrome-{version}:reader", "/auth/login")["body"]["isAdmin"])
            self.assertTrue(find(f"navidrome-{version}:admin", "/auth/login")["body"]["isAdmin"])
        for version in ["10.8.13", "10.10.7"]:
            self.assertEqual(403, find(f"jellyfin-{version}:reader", "/System/ActivityLog/Entries")["http_status"])
            self.assertFalse(find(f"jellyfin-{version}:reader", "/Users/AuthenticateByName")["body"]["User"]["Policy"]["IsAdministrator"])
            self.assertEqual("<redacted>", find(f"jellyfin-{version}:reader", "/Users/AuthenticateByName")["body"]["AccessToken"])

    def test_queue_measurement_samples_are_not_percentile_claims(self):
        import math
        report = json.loads((ROOT / "docs/app/baseline/queue-measurements.json").read_text())
        self.assertEqual(40, len(report["rows"]))
        keys = [(r["count"], r["sample"], r["operation"]) for r in report["rows"]]
        self.assertEqual(40, len(set(keys)))
        self.assertEqual({10000, 100000}, {key[0] for key in keys})
        for row in report["rows"]:
            self.assertTrue(math.isfinite(row["settledMs"]))
            self.assertGreaterEqual(row["settledMs"], row["applyMs"])
            self.assertGreaterEqual(row["applyMs"], 0)

    def test_seeded_queue_trace_integrity(self):
        report = json.loads((FIXTURES / "state-traces.json").read_text())
        trace = report["generatedTrace"]
        self.assertEqual(1000, len(trace))
        case = next(c for c in report["cases"] if c["id"] == "seeded-queue-1000")
        digest = hashlib.sha256(json.dumps(trace, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
        self.assertEqual(digest, case["observed"]["traceDigest"])
        self.assertEqual([], case["observed"]["violations"])
        self.assertEqual(list(range(1000)), [s["step"] for s in trace])

    def test_scrobble_speed_and_private_mode_evidence(self):
        report = json.loads((FIXTURES / "scrobble-traces.json").read_text())
        cases = {case["id"]: case for case in report["cases"]}
        for speed in [0.5, 1, 2]:
            case = cases[f"speed-{speed:g}"]
            first = next(i for i, state in enumerate(case["states"]) if state["submitted"])
            self.assertEqual(60 / speed, first)
            self.assertEqual(1, sum(r["query"].get("submission", False) for r in case["requests"]))
        self.assertEqual([], cases["private-mode"]["requests"])

    def test_nested_authentication_redaction(self):
        secret = "a-long-test-token-value"
        payload = {"AccessToken": secret, "nested": [{"subsonicSalt": "salt-value", "Password": "test-password"}],
                   "error": f"Bearer {secret}", "message": "invalid password"}
        clean = CAPTURE.redact(payload, [secret, "test-password"])
        self.assertNotIn(secret, json.dumps(clean))
        self.assertNotIn("salt-value", json.dumps(clean))
        self.assertEqual("invalid password", clean["message"])
        self.assertEqual("Bearer <redacted>", clean["error"])

    def test_capture_destination_is_bounded(self):
        recorder = CAPTURE.Capture({"password": "fixture"})
        for port, path in [(80, "/"), (14533, "http://other/"), (14533, "/?t=secret")]:
            with self.subTest(port=port, path=path), self.assertRaises(CAPTURE.FixtureError):
                recorder.call("fixture", port, path)

    def test_playlist_cleanup_rejects_http_and_protocol_failures(self):
        recorder = CAPTURE.Capture({"password": "fixture"})
        for result in [(500, {"status": "ok"}), (200, {"status": "failed", "error": {"message": "secret"}}),
                       (200, {})]:
            with self.subTest(result=result), patch.object(recorder, "subsonic", return_value=result) as request:
                with self.assertRaisesRegex(CAPTURE.FixtureError, "cleanup failed") as failure:
                    recorder.delete_playlist("fixture", 14533, "private-playlist-id")
                self.assertNotIn("secret", str(failure.exception))
                self.assertNotIn("private-playlist-id", str(failure.exception))
                request.assert_called_once_with("fixture", 14533, "admin", "deletePlaylist", {"id": "private-playlist-id"})

    def test_saved_live_playlist_cleanup_succeeded(self):
        report = json.loads((FIXTURES / "live-responses.json").read_text())
        deletions = [row for row in report["records"] if row["path"] == "/rest/deletePlaylist"]
        self.assertEqual(4, len(deletions))
        recorder = CAPTURE.Capture({"password": "fixture"})
        for row in deletions:
            result = row["http_status"], row["body"]["subsonic-response"]
            with self.subTest(fixture=row["fixture"]), patch.object(recorder, "subsonic", return_value=result):
                recorder.delete_playlist(row["fixture"], 14533, "fixture-playlist")

    def test_oversized_response_is_rejected(self):
        with patch.object(CAPTURE.http.client, "HTTPConnection") as connection:
            connection.return_value.getresponse.return_value.read.return_value = b"x" * (4 * 1024 * 1024 + 1)
            recorder = CAPTURE.Capture({"password": "fixture"})
            with self.assertRaisesRegex(CAPTURE.FixtureError, "4 MiB"):
                recorder.call("fixture", 14533, "/rest/ping")
            self.assertEqual([], recorder.records)
            connection.return_value.close.assert_called_once()

    @unittest.skipUnless(os.environ.get("P00_REFERENCE_ROOT") and os.environ.get("P00_ESBUILD"), "optional pinned store reference environment not configured")
    def test_pinned_store_reproduction(self):
        subprocess.run(["node", str(ROOT / "tests/reference/p00-state-traces.cjs"), os.environ["P00_REFERENCE_ROOT"],
                        os.environ["P00_ESBUILD"], str(FIXTURES / "state-traces.json")], cwd=ROOT, check=True, timeout=120, capture_output=True)

    @unittest.skipUnless(os.environ.get("P00_REFERENCE_ROOT") and os.environ.get("P00_TYPESCRIPT"), "optional pinned scrobble reference environment not configured")
    def test_pinned_scrobble_reproduction(self):
        subprocess.run(["node", str(ROOT / "tests/reference/p00-scrobble-traces.cjs"), os.environ["P00_REFERENCE_ROOT"],
                        os.environ["P00_TYPESCRIPT"], str(FIXTURES / "scrobble-traces.json")], cwd=ROOT, check=True, timeout=30, capture_output=True)


if __name__ == "__main__":
    unittest.main()
