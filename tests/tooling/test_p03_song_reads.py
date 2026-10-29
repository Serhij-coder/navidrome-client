"""Song replay provenance and integrity gates without ignored source dependencies."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests/fixtures/p03/subsonic-song-reads.json"
SCRIPT = ROOT / "tools/app/p03-song-reference.cjs"


class SongReferenceTests(unittest.TestCase):
    def test_fixture_provenance_matches_tracked_source_audit(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / "docs/app/baseline/source-audit.json").read_text())
        foundation = json.loads((ROOT / "tests/fixtures/p03/subsonic-foundation.json").read_text())
        self.assertEqual(fixture["baseline"], audit["commit"])
        self.assertEqual(set(fixture["sources"]), {
            "src/shared/api/subsonic/subsonic-normalize.ts",
            "src/shared/api/partial-iso-date.ts",
            "src/shared/types/domain-types.ts",
        })
        for name, digest in fixture["sources"].items():
            self.assertEqual(digest, audit["sources"][name])
        self.assertEqual(fixture["typescript_sha256"], foundation["typescript_sha256"])
        self.assertEqual(set(fixture["functions"]), {
            "src/shared/api/partial-iso-date.ts:3:27",
            "src/shared/api/partial-iso-date.ts:12:36",
            *{
                "src/shared/api/subsonic/subsonic-normalize.ts:" + position
                for position in ["21:23", "65:25", "101:19", "137:14", "139:31", "162:32", "173:23"]
            },
        })
        for digest in fixture["functions"].values():
            self.assertRegex(digest, r"^[0-9a-f]{64}$")
        self.assertEqual(len(fixture["cases"]), 18)
        self.assertEqual(len({case["name"] for case in fixture["cases"]}), 18)
        errors = []
        for case in fixture["cases"]:
            # Inputs are explicitly synthetic, with raw timestamp text rather
            # than a captured user profile or a live-server response.
            self.assertEqual(case["input"]["title"], "Synthetic title")
            self.assertEqual(case["input"]["created"], "raw-created-not-ISO")
            if "expected_error" in case:
                self.assertNotIn("expected", case)
                self.assertEqual(case["expected_error"], "TypeError")
                errors.append(case["input"]["contributors"][0]["role"])
            else:
                expected = case["expected"]
                self.assertEqual(expected["_serverId"], "server-a")
                self.assertEqual(expected["_serverType"], "subsonic")
                self.assertEqual(expected["_itemType"], "song")
                self.assertIsNone(expected["imageUrl"])
                self.assertEqual(expected["name"], case["input"]["title"])
        self.assertCountEqual(errors, ["__proto__", "constructor", "toString"])

    def test_tampered_source_rejected_before_compiler_loading(self):
        fixture = json.loads(FIXTURE.read_text())
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            for name in fixture["sources"]:
                target = directory / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("modified synthetic source")
            expression = (
                "try { require(process.argv[1]).replay(process.argv[2], process.argv[3]);"
                "process.exit(1); } catch (error) {"
                "process.exit(error.message === 'hash mismatch' ? 0 : 2); }"
            )
            result = subprocess.run(
                ["node", "-e", expression, str(SCRIPT), str(directory),
                 str(directory / "missing-compiler.js")],
                capture_output=True, text=True, timeout=15,
            )
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout + result.stderr, "")


if __name__ == "__main__":
    unittest.main()
