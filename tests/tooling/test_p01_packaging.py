"""Behavior checks for maintained packaging/notice tools; no tool downloads."""
import importlib.util
import gzip
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tools/app" / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


notices = load("p01_notices", "notices.py")
# package-appimage imports the sibling notices module when run as a script.
import sys
sys.modules["notices"] = notices
packaging = load("p01_packaging", "package-appimage.py")


class PackagingTests(unittest.TestCase):
    def test_runtime_libraries_include_dlopen_dependencies_and_select_native_arch(self):
        listing = """libxkbcommon-x11.so.0 (libc6) => /lib32/libxkbcommon-x11.so.0
libxkbcommon-x11.so.0 (libc6,x86-64) => /lib64/libxkbcommon-x11.so.0
libxkbcommon-x11.so.0 (libc6,AArch64) => /arm64/libxkbcommon-x11.so.0"""
        required = ("libxkbcommon-x11.so.0",)
        self.assertEqual(["/lib64/libxkbcommon-x11.so.0"], packaging.runtime_libraries(listing, "x86_64", required))
        self.assertEqual(["/arm64/libxkbcommon-x11.so.0"], packaging.runtime_libraries(listing, "aarch64", required))
        with self.assertRaisesRegex(ValueError, "install packaging runtime libraries"):
            packaging.runtime_libraries("", "x86_64", required)
        self.assertIn("libxkbcommon-x11.so.0", packaging.RUNTIME_LIBRARIES)

    def test_rejects_wrong_tool_bytes_and_arch_before_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "untrusted-tool"
            path.write_bytes(b"not the selected deployment tool")
            for architecture in ("x86_64", "aarch64", "unknown"):
                with self.assertRaises(ValueError):
                    packaging.verify_tool(path, architecture)

    def test_notice_collection_preserves_font_terms_and_rejects_missing_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crate"
            (source / "fonts").mkdir(parents=True)
            (source / "fonts/OFL.txt").write_text("Font rights and reserved names")
            (source / "Cargo.toml").write_text("[package]\nname='font-fixture'\n")
            (root / "docs/app/licenses/toolchain").mkdir(parents=True)
            original = b"<html>Exact license bytes  \n\n</html>"
            (root / "docs/app/licenses/toolchain/COPYRIGHT-library.html.gz").write_bytes(gzip.compress(original, mtime=0))
            (root / "docs/app/licenses/registry-overrides.json").write_text("{}")
            for name in ("LICENSE", "THIRD_PARTY_NOTICES.md", "Cargo.lock"):
                (root / name).write_text("fixture")
            metadata = {"packages": [{"name": "font-fixture", "version": "1", "source": "registry+fixture", "manifest_path": str(source / "Cargo.toml"), "license": "OFL-1.1"}]}
            output = root / "notices"
            notices.collect(metadata, output, root)
            self.assertEqual((output / "rust-toolchain/COPYRIGHT-library.html").read_bytes(), original)
            self.assertEqual((output / "font-fixture@1/fonts/OFL.txt").read_text(), "Font rights and reserved names")
            (source / "fonts/OFL.txt").unlink()
            with self.assertRaises(ValueError):
                notices.collect(metadata, output, root)

    def test_override_cannot_copy_files_outside_license_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "docs/app/licenses").mkdir(parents=True)
            overrides = {"fixture@1": {"files": ["../../private"]}}
            (root / "docs/app/licenses/registry-overrides.json").write_text(json.dumps(overrides))
            metadata = {"packages": [{"name": "fixture", "version": "1", "source": "registry+fixture", "manifest_path": str(root / "crate/Cargo.toml")}]}
            with self.assertRaises(ValueError):
                notices.collect(metadata, root / "output", root)
