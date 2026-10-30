"""Tracked provenance and fail-closed execution of the folder read reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-folder-reads.json'
SCRIPT = ROOT / 'tools/app/p03-folder-reference.cjs'


class FolderReferenceTests(unittest.TestCase):
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
        self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:1037:16', fixture['functions'])
        self.assertIn('src/shared/api/subsonic/subsonic-normalize.ts:457:25', fixture['functions'])
        for key, digest in song['functions'].items():
            self.assertEqual(fixture['functions'][key], digest)
        self.assertEqual(len(fixture['routes']), 2)
        self.assertEqual(fixture['routes']['getIndexes']['path'], 'getIndexes.view')
        self.assertEqual(fixture['routes']['getMusicDirectory']['path'], 'getMusicDirectory.view')
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 111)
        for prefix in ['root','directory']:
            for key in fixture['bindings']['SongListSort'].values():
                for direction in ['ASC','DESC']:
                    case = cases[prefix+'-'+key+'-'+direction]
                    self.assertEqual(case['requests'][0]['endpoint'], 'getIndexes' if prefix=='root' else 'getMusicDirectory')
                    if prefix=='root':
                        self.assertEqual(case['rng_calls'], 0)
                        self.assertEqual(case['expected']['children']['songs'], [])
        self.assertEqual([f['id'] for f in cases['root-search']['expected']['children']['folders']], ['a','a'])
        self.assertEqual(cases['directory-wrong-id']['expected']['id'], 'other')
        self.assertEqual(cases['directory-wrong-id']['target_error'], 'Stale')
        self.assertEqual(cases['directory-numeric-parent']['target_error'], 'Malformed')
        self.assertEqual([f['id'] for f in cases['unicode-folders']['expected']['children']['folders']], ['astral','private'])
        nested=cases['nested-unsorted-unfiltered']['expected']['children']['folders'][0]['children']
        self.assertEqual([f['id'] for f in nested['folders']], ['a','z'])
        self.assertEqual(len(nested['songs']), 3)

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
