"""Pinned Subsonic fixture provenance and controlled replay boundaries."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests/fixtures/p03/subsonic-foundation.json"
SCRIPT = ROOT / "tools/app/p03-subsonic-reference.cjs"


class SubsonicReferenceTests(unittest.TestCase):
    def test_fixture_is_source_bound_and_synthetic(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / "docs/app/baseline/source-audit.json").read_text())
        self.assertEqual(fixture["commit"], audit["commit"])
        self.assertEqual(fixture["kind"], "synthetic-source-replay-not-live-authentication")
        self.assertEqual(len(fixture["sources"]), 4)
        for name, digest in fixture["sources"].items():
            self.assertEqual(digest, audit["sources"][name])
        self.assertEqual(len(fixture["functions"]), 7)
        self.assertEqual(len(fixture["cases"]), 8)
        self.assertEqual(len({case["id"] for case in fixture["cases"]}), 8)
        for case in fixture["cases"]:
            expected = case["expected"]
            if not case["legacy"]:
                self.assertEqual(expected["salt"], "ABCDEFGHIJKL")
                # Independent consistency check of the captured original md5 result.
                digest = hashlib.md5((case["password"] + expected["salt"]).encode()).hexdigest()
                self.assertEqual(expected["token"], digest)
            if case["id"] == "literal-percent-username":
                self.assertEqual(expected["request_error"], "URIError")
                self.assertIsNone(expected["wire_url"])
            else:
                self.assertIsNone(expected["request_error"])
                self.assertIn("c=Feishin&f=json&v=1.13.0&u=", expected["wire_url"])

    def test_modified_source_rejected_before_compiler_loading(self):
        fixture = json.loads(FIXTURE.read_text())
        with tempfile.TemporaryDirectory() as temp:
            temp = Path(temp)
            for name in fixture["sources"]:
                target = temp / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("modified synthetic source")
            # Distinguish the integrity gate from a missing compiler/dependency error.
            expression = (
                "require(process.argv[1]).replay(process.argv[2], process.argv[3], process.argv[4])"
                ".then(() => process.exit(1), e => process.exit(e.message === 'reference hash mismatch' ? 0 : 2))"
            )
            result = subprocess.run(
                ["node", "-e", expression, str(SCRIPT), str(temp),
                 str(temp / "missing-compiler.js"), str(temp / "missing-dependencies")],
                capture_output=True, text=True, timeout=15,
            )
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout + result.stderr, "")

    @unittest.skipUnless(
        all(os.environ.get(name) for name in ("P03_SOURCE_ROOT", "P03_TYPESCRIPT", "P03_DEPENDENCIES")),
        "requires explicitly supplied pinned source/compiler/dependencies",
    )
    def test_owned_reference_replay(self):
        subprocess.run(
            ["node", str(SCRIPT), os.environ["P03_SOURCE_ROOT"], os.environ["P03_TYPESCRIPT"],
             os.environ["P03_DEPENDENCIES"]],
            check=True, timeout=30,
        )


if __name__ == "__main__":
    unittest.main()
