"""Tracked provenance and fail-closed execution of the collection reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-collection-reads.json'
SCRIPT = ROOT / 'tools/app/p03-collection-reference.cjs'


class CollectionReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 5)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 14)
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        for anchor in ['1193:25', '1390:15', '1362:24', '1502:22', '2504:1']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        self.assertEqual(len(fixture['routes']), 4)
        self.assertEqual(fixture['routes']['getSimilarSongs'], {'path':'getSimilarSongs','method':'GET'})
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 28)
        self.assertEqual(cases['roles-rich']['expected'][-2:], ['😀','\ue000'])
        self.assertEqual(cases['roles-absent']['expected'], [{'label':'all artists','value':''}])
        self.assertEqual(cases['random-rich']['request']['args']['query']['musicFolderId'], 'first /&')
        self.assertNotIn('played', cases['random-rich']['request']['args']['query'])
        self.assertNotIn('musicFolderId', cases['similar-rich']['request']['args']['query'])
        self.assertEqual([s['id'] for s in cases['similar-strict-id']['expected']], ['42'])
        self.assertEqual(cases['folders-missing']['expected_error'], 'TypeError')
        self.assertTrue(cases['roles-string-array']['target_error'])
        self.assertEqual(len(cases['roles-string-array']['expected']), 4)

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
