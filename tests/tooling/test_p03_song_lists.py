"""Tracked provenance and fail-closed execution of the song list reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-song-lists.json'
SCRIPT = ROOT / 'tools/app/p03-song-list-reference.cjs'


class SongListReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 6)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 13)
        for anchor in ['1543:18','268:1','2504:1']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        self.assertIn('src/shared/api/utils.ts:144:29', fixture['functions'])
        self.assertEqual(fixture['lodash_sha256'], '4c04561befdf653aef017a42ac5addf68ea943cdfca6bdee5ce04e04e8139f54')
        self.assertEqual(len(fixture['routes']), 5)
        self.assertEqual(len(fixture['bindings']['SongListSort']), 21)
        self.assertEqual(fixture['date_timezone'], 'UTC')
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 254)
        for prefix in ['favorite','general','search','genre','albums']:
            for key in fixture['bindings']['SongListSort'].values():
                for order in ['ASC','DESC']:
                    case = cases[prefix+'-'+key+'-'+order]
                    self.assertEqual(len(case['expected']['items']), 3)
                    if prefix != 'favorite':
                        self.assertEqual(case['rng_calls'], 0)
        ids = lambda case: [p['id'] for p in case['expected']['items']]
        self.assertEqual(ids(cases['fractional-duration']), ['b','a'])
        self.assertEqual(ids(cases['shuffle-all-max']), ['c','a','b'])
        self.assertEqual(cases['shuffle-all-max']['rng_calls'], 3)
        self.assertEqual(ids(cases['shuffle-all-zero']), ['a','b','c'])
        self.assertEqual(ids(cases['created-timezone']), ['a','b'])
        self.assertEqual([s['id'] for s in cases['created-timezone']['alternate_expected']['items']], ['b','a'])
        for name in ['created-excessive-parts','created-expanded-clip-failure','created-non-ascii']:
            self.assertEqual(cases[name]['target_error'], 'Malformed')
        self.assertEqual(cases['artist-empty']['requests'], [])
        self.assertEqual(cases['artist-fanout-repeated']['expected']['totalRecordCount'], 5)

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
