"""Tracked provenance and fail-closed execution of the enrichment reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-enrichment-reads.json'
SCRIPT = ROOT / 'tools/app/p03-enrichment-reference.cjs'


class EnrichmentReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 5)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 12)
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        for anchor in ['887:20', '954:21', '504:25']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        self.assertEqual(len(fixture['routes']), 3)
        self.assertEqual(fixture['routes']['getSimilarSongs2'], {'path':'getSimilarSongs2','method':'GET'})
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 26)
        self.assertIsNone(cases['info-http400']['expected'])
        self.assertIsNone(cases['info-http500']['expected'])
        self.assertIsNone(cases['info-absent']['expected']['similarArtists'])
        self.assertEqual(cases['info-empty']['expected']['similarArtists'], [])
        self.assertEqual(cases['info-rich']['expected']['similarArtists'][0]['userRating'], 0)
        self.assertEqual(cases['info-rich']['expected']['similarArtists'][1]['imageId'], '')
        self.assertEqual(cases['info-rich']['expected']['similarArtists'][2]['imageId'], 0)
        self.assertEqual(cases['info-missing-id']['expected']['similarArtists'][0]['id'], 'undefined')
        self.assertTrue(cases['info-missing-id']['target_error'])
        for op in ['getAlbumRadio','getArtistRadio']:
            self.assertEqual([s['id'] for s in cases[op+'-duplicates']['expected']], ['same','same','other'])

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
