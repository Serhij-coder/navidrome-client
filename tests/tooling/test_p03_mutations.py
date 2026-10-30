"""Tracked provenance and fail-closed execution of the basic mutation reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-basic-mutations.json'
SCRIPT = ROOT / 'tools/app/p03-mutation-reference.cjs'


class MutationReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 3)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 6)
        for anchor in ['374:21','430:21','2439:16','2455:23','2187:19','2149:18']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        self.assertEqual(len(fixture['routes']), 5)
        self.assertEqual(fixture['routes']['createFavorite']['path'], 'star.view')
        self.assertEqual(fixture['routes']['removeFavorite']['path'], 'unstar.view')
        self.assertEqual(fixture['routes']['getUser']['path'], 'getUser.view')
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 50)
        self.assertEqual(cases['createFavorite-album']['requests'][0]['args']['query'], {'albumId':['one/a','one/a','two & 漢']})
        self.assertEqual(cases['rating-forbidden-prefix']['expected'], None)
        self.assertEqual(len(cases['rating-forbidden-prefix']['requests']), 3)
        self.assertEqual(cases['rating-forbidden-prefix']['target_request_count'], 2)
        self.assertEqual(cases['rating-forbidden-prefix']['target_acknowledged'], 1)
        self.assertTrue(cases['rating-forbidden-prefix']['target_unknown'])
        self.assertEqual(cases['rating-invalid-later-id']['target_request_count'], 0)
        self.assertEqual(cases['rating-empty']['requests'], [])
        self.assertEqual(cases['user-admin-false']['expected']['isAdmin'], True)
        self.assertEqual(cases['user-admin-false']['target_error'], 'Malformed')
        self.assertEqual(cases['user-admin-False']['expected']['isAdmin'], False)
        self.assertEqual(cases['user-admin-True']['expected']['id'], 'canonical')
        self.assertEqual(cases['createFavorite-empty']['target_request_count'], 0)

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
