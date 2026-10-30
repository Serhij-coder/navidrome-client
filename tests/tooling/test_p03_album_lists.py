"""Tracked provenance and fail-closed execution of the album list reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-album-lists.json'
SCRIPT = ROOT / 'tools/app/p03-album-list-reference.cjs'


class AlbumListReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 7)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 15)
        for anchor in ['592:19','268:1','2504:1']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        self.assertIn('src/shared/api/utils.ts:463:30', fixture['functions'])
        self.assertEqual(fixture['lodash_sha256'], '4c04561befdf653aef017a42ac5addf68ea943cdfca6bdee5ce04e04e8139f54')
        self.assertEqual(len(fixture['routes']), 4)
        self.assertEqual(fixture['date_timezone'], 'UTC')
        self.assertEqual(fixture['bindings']['ALBUM_LIST_SORT_MAPPING']['year'], 'byYear')
        self.assertNotIn('duration', fixture['bindings']['ALBUM_LIST_SORT_MAPPING'])
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 145)
        for prefix in ['favorite','artists','general']:
            for key in fixture['bindings']['AlbumListSort'].values():
                for order in ['ASC','DESC']:
                    self.assertIn(prefix+'-'+key+'-'+order, cases)
        ids = lambda case: [p['id'] for p in case['expected']['items']]
        self.assertEqual(ids(cases['favorite-random-ASC']), ['a','c','b'])
        self.assertEqual(cases['favorite-random-ASC']['rng_calls'], 3)
        self.assertEqual(cases['general-random-ASC']['rng_calls'], 0)
        self.assertEqual(ids(cases['shuffle-all-max']), ['c','a','b'])
        self.assertEqual(ids(cases['fractional-duration']), ['earlier','later'])
        self.assertEqual(ids(cases['invalid-date-ties-DESC']), ['bad1','bad0','good'])
        self.assertEqual(cases['invalid-date-ties-DESC']['target_order'], ['bad0','bad1','good'])
        self.assertEqual([c['name'] for c in fixture['cases'] if 'target_order' in c], ['invalid-date-ties-DESC'])
        self.assertEqual(ids(cases['legacy-zone-tie']), ['positive','negative'])
        self.assertEqual([p['id'] for p in cases['legacy-zone-tie']['alternate_expected']['items']], ['negative','positive'])

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
