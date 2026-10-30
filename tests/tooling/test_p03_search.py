"""Tracked provenance and fail-closed execution of the combined search reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-search.json'
SCRIPT = ROOT / 'tools/app/p03-search-reference.cjs'


class SearchReferenceTests(unittest.TestCase):
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
        self.assertEqual(len(fixture['bindings']), 4)
        self.assertEqual(fixture['routes'], {'search3': {'method':'GET', 'path':'search3.view'}})
        for anchor in ['2391:13','2504:1']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 23)
        query = cases['independent-windows']['requests'][0]['args']['query']
        self.assertEqual(query, dict(albumCount=4, albumOffset=5, artistCount=2, artistOffset=3, songCount=6, songOffset=7, query=' Q / & + 漢 ', musicFolderId='f/a'))
        self.assertEqual(cases['all-omitted']['requests'][0]['args']['query'], {})
        rich = cases['raw-order-repeats-and-full-fields']['expected']
        self.assertEqual([s['id'] for s in rich['songs']], ['same', 'same'])
        self.assertEqual([a['id'] for a in rich['albums']], ['b', 'a'])
        self.assertEqual(len(rich['albums'][0]['songs']), 2)
        self.assertEqual(cases['song-string-release-date']['target_error'], 'Malformed')
        self.assertEqual(cases['number-parent']['expected'], dict(albumArtists=[],albums=[],songs=[]))

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
