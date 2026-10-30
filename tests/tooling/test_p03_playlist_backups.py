"""Backup statement provenance and fail-closed source execution."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/p03/subsonic-playlist-backup-source.json'
SCRIPT = ROOT / 'tools/app/p03-playlist-backup-reference.cjs'


class PlaylistBackupReferenceTests(unittest.TestCase):
    def test_provenance_and_scope_characterization(self):
        fixture = json.loads(FIXTURE.read_text())
        audit = json.loads((ROOT / 'docs/app/baseline/source-audit.json').read_text())
        mutations = json.loads((ROOT / 'tests/fixtures/p03/subsonic-playlist-mutations.json').read_text())
        self.assertEqual(fixture['baseline'], audit['commit'])
        self.assertEqual(fixture['typescript_sha256'], mutations['typescript_sha256'])
        self.assertEqual(len(fixture['sources']), 1)
        for name, digest in fixture['sources'].items():
            self.assertEqual(digest, audit['sources'][name])
        self.assertEqual(list(fixture['functions']), ['src/renderer/api/subsonic/subsonic-controller.ts:2212:22'])
        self.assertEqual(set(fixture['expressions']), {'backup', 'backupKey', 'write'})
        a, b, rejected = fixture['cases']
        self.assertNotEqual(a['server_id'], b['server_id'])
        self.assertEqual(a['expected'], b['expected'])
        self.assertEqual(a['expected'][0]['value']['songIds'], ['same', 'other', 'same'])
        self.assertEqual(a['expected'][0]['value']['timestamp'], a['captured_at_unix_ms'])
        self.assertTrue(rejected['reject_write'])
        self.assertEqual(rejected['expected_error'], 'Error')

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
