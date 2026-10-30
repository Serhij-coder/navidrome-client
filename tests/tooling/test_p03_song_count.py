"""Tracked provenance and fail-closed execution of the song count reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-song-count.json'
SCRIPT = ROOT / 'tools/app/p03-song-count-reference.cjs'


class SongCountReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 3)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 2)
        for anchor in ['1728:23', '2504:1']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        self.assertEqual(fixture['bindings'], {'MAX_SUBSONIC_ITEMS': 500, 'SUBSONIC_FAST_BATCH_SIZE': 5000})
        self.assertEqual(len(fixture['routes']), 5)
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 44)
        self.assertEqual(cases['general-5001']['expected'], 5001)
        self.assertEqual([r['args']['query']['songOffset'] for r in cases['general-5001']['requests']], [0,5000,10000,5000])
        self.assertEqual(cases['search-pages']['requests'][1]['args']['query']['songOffset'], 500)
        self.assertEqual(cases['genre-stride']['expected'], 5503)
        self.assertEqual(cases['genre-stride']['requests'][3]['args']['query']['offset'], 5000)
        self.assertNotIn('genre', cases['genre-empty-selection']['requests'][0]['args']['query'])
        self.assertEqual(cases['favorite-raw-unfiltered']['expected'], 3)
        self.assertEqual(cases['albums-and-artists-repeated']['expected'], 11)
        self.assertEqual(cases['album-artist-preferred-empty']['requests'], [])
        self.assertEqual(cases['changing-library-shrinks']['expected'], 5000)
        for name in ['album-missing-song', 'album-null-song', 'album-object-song']:
            self.assertEqual(cases[name]['expected'], 1)
            self.assertEqual(cases[name]['target_error'], 'Malformed')
        self.assertEqual(cases['artist-unsafe-id']['target_request_count'], 1)
        self.assertEqual(cases['search-header-ignored']['expected'], 2)

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
