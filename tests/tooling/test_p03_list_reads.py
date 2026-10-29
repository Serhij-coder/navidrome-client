"""Tracked provenance and fail-closed execution of the list reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-list-reads.json'
SCRIPT = ROOT / 'tools/app/p03-list-reference.cjs'


class ListReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 4)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 6)
        for anchor in ['1227:22', '1278:27', '1138:19', '268:1']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        for anchor in ['407:27', '440:24']:
            self.assertIn('src/shared/api/subsonic/subsonic-normalize.ts:' + anchor, fixture['functions'])
        self.assertEqual(fixture['lodash_sha256'], '4c04561befdf653aef017a42ac5addf68ea943cdfca6bdee5ce04e04e8139f54')
        self.assertEqual(len(fixture['routes']), 2)
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 57)
        self.assertEqual([p['id'] for p in cases['null-missing-public-ASC']['expected']['items']], ['value','null','missing'])
        self.assertEqual([p['id'] for p in cases['null-missing-public-DESC']['expected']['items']], ['missing','null','value'])
        self.assertEqual(len(cases['getPlaylistList-zero-limit']['expected']['items']), 3)
        self.assertEqual(cases['getPlaylistList-past-end']['expected']['totalRecordCount'], 4)
        self.assertEqual(cases['count-raw-null']['expected'], 2)
        self.assertEqual(cases['normalize-outside-page']['expected_error'], 'TypeError')
        for case in fixture['cases']:
            self.assertEqual(case['request']['args'], {})

    def test_tampering_precedes_compiler_execution(self):
        fixture = json.loads(FIXTURE.read_text())
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            for name in fixture['sources']:
                file = directory / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text('modified source')
            expression = (
                "require(process.argv[1]).replay(process.argv[2],process.argv[3])"
                ".then(()=>process.exit(1),e=>process.exit(e.message==='hash mismatch'?0:2))"
            )
            result = subprocess.run(['node', '-e', expression, str(SCRIPT), str(directory),
                                     str(directory / 'missing-compiler')], capture_output=True,
                                    text=True, timeout=15)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout + result.stderr, '')
