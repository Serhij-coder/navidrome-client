"""Integrity and regression checks for saved original-app UI evidence.

Live capture is optional and deliberately absent from default test dependencies.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
import zlib

ROOT = Path(__file__).resolve().parents[2]
CAPTURE = ROOT / 'tests/fixtures/p00/browser/2026-10-22'
HELPER = ROOT / 'tools/app/p00-browser-evidence.cjs'


class BrowserEvidence(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.web = json.loads((CAPTURE / 'browser-run.json').read_text())
        cls.desktop = json.loads((CAPTURE / 'desktop-run.json').read_text())
        cls.ops = {(o['fixture'], o['id']): o for o in cls.web['operations']}

    def node(self, source, *args):
        result = subprocess.run(['node', '-e', source, str(HELPER), *args], capture_output=True, text=True, timeout=10)
        self.assertEqual(0, result.returncode, result.stderr)
        return json.loads(result.stdout)

    def test_redaction_nested_serialized_auth_urls_and_request_metadata(self):
        result = self.node(r"""
const h=require(process.argv[1]);
const secrets=new Set();
h.learnSecrets({remote:{password:'generated-remote-secret'},AccessToken:'generated-auth-token'},secrets);
const clean=h.redact({password:'short',remote:{password:'generated-remote-secret'},
  serialized:JSON.stringify({credential:'u=private&s=private&t=private'}),
  url:'http://name:pass@localhost/stream?id=1&api_key=short&t=short',
  ordinary:'http://localhost',text:'prefix generated-auth-token suffix',
  list:[{'X-Emby-Token':'private-token'}]},secrets);
const request=h.requestRecord({url:()=> 'http://name:pass@localhost/rest/stream?id=1&u=user&p=pw&s=salt&t=token&api_key=key&offset=0',method:()=> 'GET',headers:()=>{throw Error('must not inspect')},postData:()=>{throw Error('must not inspect')}});
console.log(JSON.stringify({clean,request}));
""")
        clean = result['clean']
        self.assertEqual('<redacted>', clean['password'])
        self.assertEqual('<redacted>', clean['remote']['password'])
        self.assertEqual('<redacted>', json.loads(clean['serialized'])['credential'])
        self.assertEqual('prefix <redacted> suffix', clean['text'])
        self.assertEqual('http://localhost', clean['ordinary'])
        from urllib.parse import urlsplit, parse_qs
        url = urlsplit(clean['url'])
        self.assertIsNone(url.username)
        self.assertEqual({'id': ['1'], 'api_key': ['<redacted>'], 't': ['<redacted>']}, parse_qs(url.query))
        self.assertEqual({'method': 'GET', 'origin': 'http://localhost', 'path': '/rest/stream', 'queryKeys': ['id', 'offset']}, result['request'])
        self.assertEqual('<redacted>', clean['list'][0]['X-Emby-Token'])

    def test_capture_roots_reject_overlap_and_public_credentials(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'private').mkdir()
            result = self.node("""
const h=require(process.argv[1]),p=require('node:path'),r=process.argv[2];
const cases=[['same','same','creds'],['private','private/public','creds'],['public/private','public','creds'],['private','public','public/credentials.json']];
const errors=cases.map(args=>{try{h.verifyOutputSeparation(...args.map(a=>p.join(r,a)));return null}catch(e){return e.message}});
h.verifyOutputSeparation(p.join(r,'private'),p.join(r,'public'),p.join(r,'credentials.json'));
Promise.all(['p00-browser-capture.cjs','p00-desktop-capture.cjs'].map(async file=>{
 try{await require(p.join(process.argv[3],file)).capture({privateRoot:p.join(r,'fresh'),outputRoot:p.join(r,'fresh'),credentialsFile:p.join(r,'creds')});return false}
 catch(e){return e.message==='Private and public capture roots must be disjoint'}
})).then(preflight=>console.log(JSON.stringify({errors,preflight,created:require('node:fs').existsSync(p.join(r,'fresh'))})));
""", directory, str(ROOT / 'tests/reference'))
            self.assertEqual(['Private and public capture roots must be disjoint'] * 3 + ['Credentials must remain outside public capture output'], result['errors'])
            self.assertEqual([True, True], result['preflight'])
            self.assertFalse(result['created'])

    def test_capture_roots_resolve_symlinked_ancestors(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'private').mkdir()
            (root / 'public').mkdir()
            (root / 'credentials.json').write_text('{}')
            try:
                (root / 'alias').symlink_to(root / 'private', target_is_directory=True)
                (root / 'public/private-alias').symlink_to(root / 'private', target_is_directory=True)
                (root / 'public/credentials-alias').symlink_to(root / 'credentials.json')
            except (OSError, NotImplementedError):
                self.skipTest('directory symlink creation unavailable; ordinary path guards still tested')
            result = self.node("""
const h=require(process.argv[1]),p=require('node:path'),r=process.argv[2];
const cases=[['private','alias/output','creds'],['public/private-alias','public','creds'],['private','public','public/credentials-alias']];
console.log(JSON.stringify(cases.map(args=>{try{h.verifyOutputSeparation(...args.map(a=>p.join(r,a)));return null}catch(e){return e.message}})));
""", directory)
            self.assertEqual(['Private and public capture roots must be disjoint'] * 2 + ['Credentials must remain outside public capture output'], result)

    def test_source_mismatch_rejected_before_browser_dependency_load(self):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, 'source.ts').write_text('changed')
            result = self.node("""
const h=require(process.argv[1]);
try {h.verifySource(process.argv[2],{commit:h.COMMIT,sources:{'source.ts':'0'.repeat(64)}});process.exitCode=2}
catch(e){console.log(JSON.stringify(e.message))}
""", directory)
        self.assertEqual('Pinned source hash mismatch: source.ts', result)

    def test_all_screenshot_and_dom_receipts_png_crc_and_dimensions(self):
        listed = set()
        audit = hashlib.sha256((ROOT / 'docs/app/baseline/source-audit.json').read_bytes()).hexdigest()
        for report in [self.web, self.desktop]:
            self.assertEqual(audit, report['sourceAuditSha256'])
            self.assertEqual('7d6ad318c79802e226996cfc39acba6b05bb22e8', report['commit'])
            for item in report['screenshots']:
                for file_key, hash_key in [('file', 'sha256'), ('domFile', 'domSha256')]:
                    name = item[file_key]
                    self.assertEqual(name, Path(name).name)
                    self.assertNotIn(name, listed)
                    listed.add(name)
                    self.assertEqual(item[hash_key], hashlib.sha256((CAPTURE / name).read_bytes()).hexdigest())
                data = (CAPTURE / item['file']).read_bytes()
                self.assertEqual(b'\x89PNG\r\n\x1a\n', data[:8])
                width, height = struct.unpack('>II', data[16:24])
                expected = item.get('viewport', report['environment'].get('viewport'))
                self.assertEqual((expected['width'], expected['height']), (width, height))
                offset = 8
                while offset < len(data):
                    size = struct.unpack('>I', data[offset:offset + 4])[0]
                    chunk = data[offset + 4:offset + 8 + size]
                    crc = struct.unpack('>I', data[offset + 8 + size:offset + 12 + size])[0]
                    self.assertEqual(crc, zlib.crc32(chunk))
                    offset += 12 + size
                self.assertEqual(len(data), offset)
        self.assertEqual(51, len(listed) // 2)
        self.assertEqual({p.name for p in CAPTURE.glob('*.png')}, {p for p in listed if p.endswith('.png')})

    def test_six_live_ui_logins_discovery_and_observed_operations(self):
        versions = {'navidrome-current': '0.61.0', 'navidrome-old': '0.47.5', 'gonic-current': '0.16.4', 'gonic-old': '0.15.2', 'jellyfin-current': '10.10.7', 'jellyfin-old': '10.8.13'}
        self.assertEqual(28, len(self.ops))
        self.assertTrue(all(o['status'] == 'observed' for o in self.ops.values()))
        for fixture, version in versions.items():
            if fixture == 'gonic-old':
                self.assertIsNone(self.web['serverDiscovery'][fixture]['serverVersion'])
                self.assertFalse(self.web['serverDiscovery'][fixture]['openSubsonic'])
            else:
                self.assertIn(version, self.web['serverDiscovery'][fixture]['serverVersion'])
            for operation in ['login', 'tracks', 'albums']:
                self.assertIn((fixture, operation), self.ops)
                self.assertTrue(any(r['fixture'] == fixture and r['operation'] == operation and r['status'] == 200 for r in self.web['requests']))
        self.assertEqual([], self.web['pageErrors'])
        self.assertGreater(len(self.web['servedAssets']), 50)
        for request in self.web['requests']:
            self.assertNotIn('headers', request)
            self.assertNotIn('body', request)
            self.assertFalse({'u', 'p', 's', 't', 'api_key', 'X-Emby-Token'} & set(request['queryKeys']))

    def test_duplicate_drag_reload_and_actual_media_pause(self):
        before = json.loads((CAPTURE / 'browser-queue.json').read_text())
        after = json.loads((CAPTURE / 'browser-queue-after-drag.json').read_text())
        ids = before['default']
        self.assertEqual(3, len(set(ids)))
        self.assertEqual(before['songs'][ids[0]]['id'], before['songs'][ids[1]]['id'])
        self.assertEqual([ids[2], ids[0], ids[1]], after['default'])
        reload = self.ops[('navidrome-current', 'queue-reload')]['observed']
        self.assertEqual(after['default'], reload['beforeIds'])
        self.assertEqual(after['default'], reload['afterIds'])
        playback = self.ops[('navidrome-current', 'browser-playback-and-space-pause')]['observed']
        self.assertTrue(any(not a['paused'] and a['currentTimeSec'] > 0.25 for a in playback['before']))
        self.assertTrue(playback['after'])
        self.assertTrue(all(a['paused'] for a in playback['after']))
        self.assertTrue(any(0 < a['currentTimeSec'] < a['durationSec'] and not a['ended'] for a in playback['after'] if a['durationSec']))

    def test_current_export_import_reload_and_sensitive_settings_removed(self):
        observed = self.ops[('navidrome-current', 'settings-export-import-reload')]['observed']
        self.assertEqual(34, observed['version'])
        self.assertEqual(['store_settings'], observed['storageKeys'])
        self.assertFalse(observed['appliedResume'])
        self.assertFalse(observed['reloadedResume'])
        self.assertTrue(observed['currentVersionOnly'])
        for key in ['downloadedFile', 'afterImportFile', 'afterReloadFile']:
            value = json.loads((CAPTURE / observed[key]).read_text())
            self.assertEqual('<redacted>', value['remote']['password'])
            self.assertEqual(34, value['version'])
        self.assertEqual(observed['downloadedSha256'], hashlib.sha256((CAPTURE / observed['downloadedFile']).read_bytes()).hexdigest())

    def test_desktop_identity_guard_and_remote_correlated_ack(self):
        env = self.desktop['environment']
        self.assertEqual('1.17.0', env['runtime']['version'])
        self.assertEqual('43.7.0', env['runtime']['electron'])
        self.assertEqual({'disabled': True, 'optOut': '1'}, env['analytics'])
        self.assertFalse(env['nativeGpu'])
        self.assertFalse(env['browserSandbox'])
        self.assertEqual([{'address': '127.0.0.1', 'family': 'IPv4', 'port': 18433}], env['remoteListeners'])
        self.assertTrue(all(o['status'] == 'observed' for o in self.desktop['operations']))
        operation = next(o for o in self.desktop['operations'] if o['id'] == 'remote-auth-queue-clear-ack')
        request = next(e for e in self.desktop['socketEvents'] if e['direction'] == 'sent' and e['event'] == 'clear-queue')
        ack = next(e for e in self.desktop['socketEvents'] if e['direction'] == 'received' and e['event'] == 'operation-ack' and e['requestId'] == request['requestId'])
        self.assertEqual(operation['requestId'], request['requestId'])
        self.assertIsNone(ack['error'])
        self.assertTrue(operation['desktopQueuePresent'])
        self.assertEqual(0, operation['desktopQueueCount'])
        settings = next(s for s in self.desktop['screenshots'] if s['id'] == 'desktop-window-settings')
        self.assertGreaterEqual(settings['maskedElementCount'], 3)
        for screenshot in self.desktop['screenshots']:
            if screenshot['id'].startswith('remote-'):
                self.assertEqual({'width': 390, 'height': 844}, screenshot['viewport'])


if __name__ == '__main__':
    unittest.main()
