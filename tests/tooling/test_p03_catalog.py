"""Tracked provenance and fail-closed execution of the catalog read reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-catalog-reads.json'
SCRIPT = ROOT / 'tools/app/p03-catalog-reference.cjs'


class CatalogReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 7)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 19)
        for anchor in ['987:23','1180:31','1416:20','2099:18','1543:18']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        self.assertIn('src/shared/api/utils.ts:46:27', fixture['functions'])
        self.assertEqual(len(fixture['routes']), 8)
        self.assertEqual(fixture['routes']['getTopSongsList']['path'], 'getTopSongs.view')
        self.assertEqual(fixture['bindings']['ServerFeature']['TOP_SONGS_BY_ARTIST_ID'], 'topSongsByArtistId')
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 50)
        ids = lambda c: [s['id'] for s in c['expected']['items']]
        self.assertEqual(ids(cases['favorite-favorite-1']), ['unrated','high','high'])
        self.assertEqual(ids(cases['favorite-rating-1']), ['high','high','rated'])
        self.assertEqual(cases['favorite-rating-1']['expected']['totalRecordCount'], 6)
        self.assertEqual(cases['personal-1']['expected']['totalRecordCount'], 6)
        self.assertEqual(len(cases['personal-1']['expected']['items']), 5)
        self.assertEqual(cases['community-[42]-None']['requests'][0]['args']['query'], {'id':'id/a'})
        self.assertEqual(cases['community-[]-None']['requests'][0]['args']['query'], {'artist':' Name & / 漢 '})
        self.assertEqual(len(cases['community-None-0']['expected']['items']), 6)
        self.assertEqual(cases['stations-full']['expected'][0]['imageId'], '0')
        self.assertEqual(cases['stations-full']['expected'][1]['homepageUrl'], None)
        self.assertEqual(cases['scan-negative']['target_error'], 'Malformed')
        self.assertEqual(cases['scan-missing']['target_error'], 'Malformed')
        self.assertTrue(all(c['rng_calls']==0 for c in fixture['cases']))

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
