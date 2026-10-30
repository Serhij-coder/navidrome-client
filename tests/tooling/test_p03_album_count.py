"""Tracked provenance and fail-closed execution of the album count reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-album-count.json'
SCRIPT = ROOT / 'tools/app/p03-album-count-reference.cjs'


class AlbumCountReferenceTests(unittest.TestCase):
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
        for anchor in ['743:24','2504:1']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        self.assertEqual(fixture['bindings']['MAX_SUBSONIC_ITEMS'], 500)
        self.assertEqual(fixture['bindings']['AlbumListSortType']['BY_YEAR'], 'byYear')
        self.assertEqual(len(fixture['routes']), 4)
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 40)
        self.assertEqual(cases['pages500-then2']['expected'], 502)
        self.assertEqual(cases['pages500-then2']['requests'][1]['args']['query']['offset'], 500)
        self.assertEqual(cases['search-pages']['requests'][1]['args']['query']['albumOffset'], 500)
        self.assertEqual(cases['artist-empty-priority']['requests'], [])
        self.assertEqual(cases['artist-http-reset']['expected'], 1)
        self.assertEqual(cases['header-second-page']['expected'], 123)
        self.assertEqual(cases['header-http-rejected']['expected'], 123)
        self.assertEqual(cases['header-http-rejected']['target_error'], 'Http')
        self.assertEqual(cases['header-protocol-rejected']['expected'], 123)
        self.assertEqual(cases['header-protocol-rejected']['target_error'], 'Protocol')
        self.assertEqual(cases['header-hex']['expected'], 16)
        self.assertEqual(cases['header-hex']['target_error'], 'Malformed')

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
