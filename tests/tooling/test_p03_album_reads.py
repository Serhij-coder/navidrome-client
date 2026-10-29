"""Tracked provenance and fail-closed execution of the album reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-album-reads.json'
SCRIPT = ROOT / 'tools/app/p03-album-reference.cjs'


class AlbumReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 4)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 14)
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:577:21', fixture['functions'])
        self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:483:27', fixture['functions'])
        cases = {case['name']: case for case in fixture['cases']}
        self.assertNotIn('sortName', cases['minimal-without-title']['expected'])
        self.assertIsNone(cases['null-songs-and-title']['expected']['sortName'])
        self.assertIsNone(cases['invalid-count-source-nan']['expected']['albumCount'])
        self.assertTrue(cases['invalid-count-source-nan']['target_error'])
        self.assertTrue(cases['invalid-disc-fraction']['target_error'])
        self.assertEqual(cases['inherited-contributor']['expected_error'], 'TypeError')
        self.assertEqual(cases['null-similar-entry']['expected_error'], 'TypeError')
        for case in fixture['cases']:
            self.assertEqual(case['request']['endpoint'], 'getAlbum' if case['kind'] == 'album' else 'getArtist')
            self.assertEqual(case['request']['id'], case['input']['id'])

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
