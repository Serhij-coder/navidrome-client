"""Native notice provenance using synthetic ELF identities and dpkg responses."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('native_notices_test', Path(__file__).resolve().parents[2] / 'tools/app/native_notices.py')
native = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(native)


class NativeNoticesTests(unittest.TestCase):
    def fixture(self, root):
        app = root / 'AppDir'
        (app / 'usr/bin').mkdir(parents=True)
        (app / 'usr/lib').mkdir()
        binary = root / 'feishin-desktop'
        binary.write_bytes(b'ELF original project')
        (app / 'usr/bin/feishin-desktop').write_bytes(b'ELF patched project')
        (app / 'AppRun').symlink_to('usr/bin/feishin-desktop')
        source = root / 'libfixture.so.1'
        source.write_bytes(b'ELF original library')
        (app / 'usr/lib/libfixture.so.1').write_bytes(b'ELF patched library')
        copyright_root = root / 'copyrights'
        (copyright_root / 'fixture').mkdir(parents=True)
        (copyright_root / 'fixture/copyright').write_bytes(b'Exact rights\r\n  preserved\n')
        return app, binary, source, copyright_root

    @staticmethod
    def elf(path):
        if path.read_bytes().startswith(b'ELF'):
            return {'machine': 'fixture-machine', 'build_id': 'app' if path.name == 'feishin-desktop' else 'library'}
        return None

    @staticmethod
    def command(args):
        if args[:2] == ['dpkg-query', '--version']:
            return 'fixture dpkg'
        if args[:2] == ['dpkg-query', '-S']:
            return 'fixture:amd64: ' + args[-1]
        if args[:2] == ['dpkg-query', '-W']:
            return 'fixture:amd64\t1.2-3\tfixture-source\t1.2-3\tamd64'
        raise AssertionError(args)

    def collect(self, app, binary, source, copyrights):
        return native.collect(app, binary, app / 'usr/share/doc/native', f'libfixture.so.1 => {source}', 'tool-digest', copyrights)

    def test_exact_package_and_copyright_with_patched_elf_and_app_symlink(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(native, 'elf', self.elf), patch.object(native, 'command', self.command):
            app, binary, source, copyrights = self.fixture(Path(directory))
            manifest = self.collect(app, binary, source, copyrights)
            library = next(row for row in manifest['files'] if row['kind'] == 'distribution')
            self.assertNotEqual(library['source_sha256'], library['bundled_sha256'])
            self.assertEqual(library['package']['source_package'], 'fixture-source')
            self.assertEqual(library['package']['binary_version'], '1.2-3')
            self.assertEqual((app / 'usr/share/doc/native/fixture_amd64/copyright').read_bytes(), b'Exact rights\r\n  preserved\n')
            native.verify(app, manifest)
            (app / 'usr/lib/libfixture.so.1').write_bytes(b'ELF later changed')
            with self.assertRaisesRegex(ValueError, 'changed during finalization'):
                native.verify(app, manifest)

    def test_missing_and_ambiguous_sources_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(native, 'elf', self.elf), patch.object(native, 'command', self.command):
            app, binary, source, copyrights = self.fixture(Path(directory))
            with self.assertRaisesRegex(ValueError, 'unmapped or ambiguous'):
                native.collect(app, binary, app / 'notices', '', 'tool', copyrights)
            other = Path(directory) / 'other/libfixture.so.1'
            other.parent.mkdir()
            other.write_bytes(source.read_bytes())
            with self.assertRaisesRegex(ValueError, 'unmapped or ambiguous'):
                native.collect(app, binary, app / 'notices', f'libfixture.so.1 => {source}\nlibfixture.so.1 => {other}', 'tool', copyrights)

    def test_escaping_symlink_and_missing_dpkg_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(native, 'elf', self.elf), patch.object(native, 'command', self.command):
            app, binary, source, copyrights = self.fixture(Path(directory))
            (app / 'escape').symlink_to(source)
            with self.assertRaisesRegex(ValueError, 'escapes staging'):
                self.collect(app, binary, source, copyrights)
            (app / 'escape').unlink()
        with patch.object(native.subprocess, 'check_output', side_effect=FileNotFoundError()):
            with self.assertRaisesRegex(ValueError, 'native provenance command failed: dpkg-query'):
                native.command(['dpkg-query', '--version'])

    def test_missing_copyright_and_metadata_rejected(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(native, 'elf', self.elf), patch.object(native, 'command', self.command):
            app, binary, source, copyrights = self.fixture(Path(directory))
            (copyrights / 'fixture/copyright').unlink()
            with self.assertRaisesRegex(ValueError, 'missing exact package copyright'):
                self.collect(app, binary, source, copyrights)
        with patch.object(native, 'command', return_value=''):
            with self.assertRaisesRegex(ValueError, 'missing dpkg owner'):
                native.package(Path('/missing'))
