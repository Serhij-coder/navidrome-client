"""Tracked provenance and fail-closed execution of the artist reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-artist-lists.json'
SCRIPT = ROOT / 'tools/app/p03-artist-reference.cjs'


class ArtistReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 5)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 8)
        for anchor in ['533:25','572:30','909:20','949:25','268:1','2504:1']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        self.assertIn('src/shared/api/subsonic/subsonic-normalize.ts:271:30', fixture['functions'])
        self.assertIn('src/shared/api/utils.ts:430:36', fixture['functions'])
        self.assertEqual(fixture['lodash_sha256'], '4c04561befdf653aef017a42ac5addf68ea943cdfca6bdee5ce04e04e8139f54')
        self.assertEqual(len(fixture['routes']), 1)
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 83)
        original = ['1','2','3','1']
        for order in ['ASC','DESC']:
            self.assertEqual([p['id'] for p in cases['getAlbumArtistList-favorited-'+order]['expected']['items']], original)
        self.assertEqual([p['id'] for p in cases['getArtistList-albumCount-DESC']['expected']['items']], ['2','3','1','1'])
        self.assertEqual(cases['getArtistListCount-past-end']['expected'], 4)
        self.assertEqual([p['id'] for p in cases['role-before-normalize']['expected']['items']], ['1'])
        self.assertTrue(cases['normalize-before-search']['target_error'])
        self.assertEqual(cases['similar-rich']['expected']['items'][0]['similarArtists'][0]['userRating'], None)
        self.assertTrue(cases['malformed-role-type']['target_error'])

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
