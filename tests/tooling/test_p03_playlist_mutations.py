"""Tracked provenance and fail-closed execution of the playlist mutation reference tool."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-playlist-mutations.json'
SCRIPT = ROOT / 'tools/app/p03-playlist-mutation-reference.cjs'


class PlaylistMutationReferenceTests(unittest.TestCase):
    def test_provenance_and_characterized_differences(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        song = json.loads((ROOT / 'tests/fixtures/p03/subsonic-song-reads.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], song['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 2)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(len(fixture['functions']), 6)
        for anchor in ['302:20','414:21','468:21','2198:25','2423:23','2484:21']:
            self.assertIn('src/renderer/api/subsonic/subsonic-controller.ts:' + anchor, fixture['functions'])
        self.assertEqual(len(fixture['routes']), 3)
        self.assertEqual(fixture['routes']['createPlaylist']['path'], 'createPlaylist.view')
        self.assertEqual(fixture['routes']['updatePlaylist']['path'], 'updatePlaylist.view')
        cases = {case['name']: case for case in fixture['cases']}
        self.assertEqual(len(cases), 38)
        self.assertEqual(cases['create-name-only']['requests'][0]['args']['query'], {'name':' Name & / 漢 '})
        self.assertEqual(cases['create-name-only']['expected'], {'id':'42','name':'Canonical'})
        self.assertEqual(cases['metadata-False-']['requests'][0]['args']['query'], {'comment':'','name':'Name','playlistId':'p','public':False})
        self.assertEqual(cases['add-3']['requests'][0]['args']['query']['songIdToAdd'], ['same','same','other & / 漢'])
        self.assertEqual(cases['set-0']['requests'][0]['args']['query'], {'playlistId':'p','songId':[]})
        self.assertEqual(cases['remove-3']['requests'][0]['args']['query']['songIndexToRemove'], ['2','2','0'])
        self.assertEqual(cases['create-missing-response-id']['target_acknowledged'], 1)
        self.assertFalse(cases['create-missing-response-id']['target_unknown'])
        self.assertEqual(cases['delete-blank']['target_request_count'], 0)

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
