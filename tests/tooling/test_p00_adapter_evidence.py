"""Recorded adapter evidence checks; original-source reproduction is optional."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/fixtures/p00"
REFERENCE = os.environ.get("P00_REFERENCE_ROOT")
ESBUILD = os.environ.get("P00_ESBUILD")


class AdapterEvidence(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.report = json.loads((FIXTURES / "adapter-traces.json").read_text())
        cls.cases = {case["id"]: case for case in cls.report["cases"]}

    def test_live_replay_provenance_and_explicit_synthetic_inputs(self):
        self.assertEqual("pinned-adapters-with-offline-transport", self.report["evidenceKind"])
        live_bytes = (FIXTURES / "live-responses.json").read_bytes()
        self.assertEqual(hashlib.sha256(live_bytes).hexdigest(), self.report["liveCaptureSha256"])
        self.assertEqual(hashlib.sha256((FIXTURES / "scrobble-traces.json").read_bytes()).hexdigest(), self.report["scrobbleCaptureSha256"])
        records = json.loads(live_bytes)["records"]
        for case in self.cases.values():
            for sample in case["inputs"]:
                source = sample["source"]
                self.assertIn(source["kind"], {"saved-live-response", "synthetic"})
                if source["kind"] == "saved-live-response":
                    row = records[source["record"]]
                    self.assertEqual(row["http_status"], sample["status"])
                    path = row["path"] + ".view" if row["path"].startswith("/rest/") else row["path"].lower()
                    self.assertEqual(path, sample["path"])
        self.assertEqual(len(self.cases), len(self.report["cases"]))
        self.assertTrue(self.report["provenance"]["responseValidation"].startswith("enabled"))

    def test_six_versions_pagination_and_duplicate_entries(self):
        for fixture in ["navidrome-0.61.0", "navidrome-0.47.5", "gonic-0.16.4", "gonic-0.15.2"]:
            with self.subTest(fixture=fixture):
                first = self.cases[fixture + "-search-offset-0"]["observed"]
                self.assertEqual([3000, 4000], [song["durationMs"] for song in first["songs"]])
                self.assertEqual({fixture}, {song["serverId"] for song in first["songs"]})
                self.assertIsNone(first["totalRecordCount"])
                self.assertEqual([], self.cases[fixture + "-search-offset-2"]["observed"]["songs"])
                duplicates = self.cases[fixture + "-playlist-duplicates"]["observed"]["items"]
                self.assertEqual(2, len(duplicates))
                self.assertEqual(duplicates[0]["id"], duplicates[1]["id"])
                self.assertNotEqual(duplicates[0]["playlistItemId"], duplicates[1]["playlistItemId"])
                self.assertEqual(1, len(self.cases[fixture + "-playlist-after-removal"]["observed"]["items"]))
        for fixture in ["jellyfin-10.10.7", "jellyfin-10.8.13"]:
            self.assertEqual(2, self.cases[fixture + "-songs-offset-2"]["observed"]["totalRecordCount"])
            self.assertEqual([], self.cases[fixture + "-songs-offset-2"]["observed"]["songs"])
        for version in ["0.61.0", "0.47.5"]:
            self.assertEqual(0, self.cases[f"navidrome-{version}-native-songs-missing-count-header"]["observed"]["totalRecordCount"])

    def test_protocol_errors_silent_mode_and_cancellation_remain_distinct(self):
        for fixture in ["navidrome-0.61.0", "navidrome-0.47.5", "gonic-0.16.4", "gonic-0.15.2"]:
            case = self.cases[fixture + "-invalid-auth-envelope"]
            self.assertEqual(40, case["observed"]["clientStatus"])
            self.assertEqual(1, case["authFailures"])
        self.assertEqual(70, self.cases["old-gonic-missing-extension-envelope"]["observed"]["clientStatus"])
        self.assertEqual({"clientStatus": 200, "protocolStatus": "failed", "protocolCode": 0}, self.cases["silent-subsonic-error-rewrites-code"]["observed"])
        self.assertEqual("error.networkError", self.cases["subsonic-network"]["observed"]["error"])
        self.assertIsNone(self.cases["subsonic-cancellation"]["observed"]["clientStatus"])
        self.assertEqual([], self.cases["subsonic-pre-aborted-request"]["requests"])
        self.assertFalse(self.cases["malformed-subsonic-envelope"]["observed"]["validationReached"])

    def test_callback_to_adapter_submission_and_bounded_queue(self):
        for backend in ["subsonic", "navidrome", "jellyfin"]:
            observed = self.cases[backend + "-submission-transport-boundary"]["observed"]
            self.assertEqual(backend == "jellyfin", observed["resolvedBeforeTransport"])
            self.assertEqual(1800000000000 if backend == "jellyfin" else None, observed["outgoingSubmissionPosition"])
        self.assertEqual("Failed to scrobble", self.cases["subsonic-rejected-submission"]["observed"]["error"])
        for event in ["start", "unpause"]:
            queue = self.cases["jellyfin-queue-window-" + event]["observed"]["reportedQueue"]
            self.assertEqual(20, len(queue))
            self.assertEqual(queue[1], queue[2])
        for event in ["stop", "timeupdate"]:
            self.assertIsNone(self.cases["jellyfin-queue-window-" + event]["observed"]["reportedQueue"])

    def test_recorded_requests_omit_authentication_material(self):
        for case in self.cases.values():
            for request in case["requests"]:
                self.assertTrue(request["authenticationPresent"])
                self.assertFalse(set(request["query"]) & {"u", "p", "s", "t"})
                self.assertNotIn("headers", request)
                if isinstance(request.get("body"), str):
                    from urllib.parse import parse_qs
                    self.assertFalse(set(parse_qs(request["body"])) & {"u", "p", "s", "t"})
        text = json.dumps(self.report)
        self.assertNotIn("fixture-native", text)
        self.assertNotIn("u=fixture", text)

    def test_modified_source_rejected_before_loading_dependencies(self):
        audit = json.loads((ROOT / "docs/app/baseline/source-audit.json").read_text())
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / next(iter(audit["sources"]))
            source.parent.mkdir(parents=True)
            source.write_text("changed source")
            script = "require(process.argv[1]).loadAdapters(process.argv[2], '/missing-dependency').then(()=>process.exit(2), e=>{if(!e.message.startsWith('Pinned reference hash mismatch:'))process.exit(3)})"
            result = subprocess.run(["node", "-e", script, str(ROOT / "tools/app/p00-adapter-runtime.cjs"), directory], capture_output=True, text=True, timeout=10)
            self.assertEqual(0, result.returncode, result.stderr)

    def test_hash_bound_fixtures_survive_autocrlf_checkout(self):
        paths = [Path("tests/fixtures/p00/live-responses.json"), Path("tests/fixtures/p00/scrobble-traces.json")]
        with tempfile.TemporaryDirectory() as directory:
            checkout = Path(directory)
            shutil.copy2(ROOT / ".gitattributes", checkout / ".gitattributes")
            for path in paths:
                (checkout / path).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(ROOT / path, checkout / path)
            control = checkout / "unpinned.txt"
            control.write_bytes(b"checkout conversion control\n")
            git = ["git", "-c", "core.autocrlf=true", "-c", "core.attributesFile=" + os.devnull]
            environment = os.environ | {"GIT_ATTR_NOSYSTEM": "1"}
            for command in [git + ["init", "--quiet"], git + ["add", ".gitattributes", "unpinned.txt", *map(str, paths)]]:
                subprocess.run(command, cwd=checkout, env=environment, check=True, timeout=10, capture_output=True)
            # Force a fresh checkout; checkout-index can otherwise retain matching working files.
            for path in paths:
                (checkout / path).unlink()
            control.unlink()
            subprocess.run(git + ["checkout-index", "--all"], cwd=checkout, env=environment, check=True, timeout=10, capture_output=True)
            for path in paths:
                self.assertEqual((ROOT / path).read_bytes(), (checkout / path).read_bytes())
            self.assertEqual(b"checkout conversion control\r\n", control.read_bytes())

    @unittest.skipUnless(REFERENCE and ESBUILD, "optional pinned adapter reference environment not configured")
    def test_exact_pinned_adapter_reproduction(self):
        subprocess.run(["node", str(ROOT / "tests/reference/p00-adapter-traces.cjs"), REFERENCE, ESBUILD,
                        str(FIXTURES / "adapter-traces.json")], cwd=ROOT, check=True, timeout=60, capture_output=True)

    @unittest.skipUnless(REFERENCE and ESBUILD, "optional pinned adapter reference environment not configured")
    def test_transport_rejects_nonfixture_origin(self):
        script = """
const {loadAdapters}=require(process.argv[1]);
loadAdapters(process.argv[2],process.argv[3]).then(async ({api})=>{
  const server={url:'http://127.0.0.1:14533',credential:'u=fixture&s=fixture&t=fixture',username:'fixture'};
  try { await api.ssApiClient({server}).ping(); process.exitCode=2; }
  catch(e) { if(e.message!=='Only the offline fixture origin is allowed')process.exitCode=3; }
}).catch(()=>{process.exitCode=4});
"""
        result = subprocess.run(["node", "-e", script, str(ROOT / "tools/app/p00-adapter-runtime.cjs"), REFERENCE, ESBUILD], capture_output=True, text=True, timeout=30)
        self.assertEqual(0, result.returncode, result.stderr)


if __name__ == "__main__":
    unittest.main()
