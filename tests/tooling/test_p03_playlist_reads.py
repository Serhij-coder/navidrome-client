"""Tracked provenance and fail-closed execution of the playlist reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-playlist-reads.json'
SCRIPT = ROOT / 'tools/app/p03-playlist-reference.cjs'


class PlaylistReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 4)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 13)
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        for anchor in ['1212:24', '1302:26', '1297:25']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        self.assertIn('src/shared/api/subsonic/subsonic-normalize.ts:407:27', fixture['functions'])
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 25)
        repeated = cases['repeated-songs']['expected']
        self.assertEqual(repeated['totalRecordCount'], 3)
        self.assertEqual([s['playlistItemId'] for s in repeated['items']], ['0','1','2'])
        self.assertEqual(repeated['items'][0]['id'], repeated['items'][1]['id'])
        self.assertEqual(cases['ids-repeated']['expected']['items'], ['same','same'])
        self.assertEqual(cases['full']['expected']['imageId'], '42&_=raw changed &=?')
        self.assertEqual(cases['entries-malformed-array']['expected_error'], 'TypeError')
        self.assertTrue(cases['entries-missing-parent-id']['target_error'])
        for case in fixture['cases']:
            self.assertEqual(case['request']['endpoint'], 'getPlaylist')
            self.assertEqual(case['request']['id'], case['request_id'])

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
