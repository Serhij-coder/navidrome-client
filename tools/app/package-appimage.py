#!/usr/bin/env python3
"""Create a development AppImage; no publishing, signing or release certification."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
from notices import ROOT, collect

# linuxdeploy build 07333c6, 2026-10-01. Mutable download aliases fail closed
# when bytes change; an update requires explicit review of this manifest.
LINUXDEPLOY_SHA256 = {
    "x86_64": "8aea8da0f7f7039d2a2cecb14657d752a222a5e1d3825caeef186c82f751cdd1",
    "aarch64": "5c1fddf96066891e829831cac0d84424690f3b22846c7f8f1bb9990a5c6c73f4",
}

# winit/clipboard load these at runtime; an ELF dependency scan cannot see them.
RUNTIME_LIBRARIES = (
    "libxkbcommon.so.0", "libxkbcommon-x11.so.0", "libX11.so.6", "libXcursor.so.1",
    "libXi.so.6", "libXrandr.so.2", "libXfixes.so.3", "libXrender.so.1", "libxcb.so.1",
    "libwayland-client.so.0", "libwayland-cursor.so.0", "libwayland-egl.so.1",
)


def runtime_libraries(listing, architecture, required=RUNTIME_LIBRARIES):
    marker = {"x86_64": "x86-64", "aarch64": "aarch64"}.get(architecture)
    if marker is None:
        raise ValueError("unsupported native library architecture")
    found = {}
    for line in listing.splitlines():
        if "=>" not in line:
            continue
        description, path = line.split("=>", 1)
        name = description.strip().split()[0]
        if name in required and marker in description.lower():
            found.setdefault(name, path.strip())
    missing = set(required) - found.keys()
    if missing:
        raise ValueError("install packaging runtime libraries: " + ", ".join(sorted(missing)))
    return [found[name] for name in required]


def verify_tool(path, architecture):
    with path.open("rb") as handle:
        digest = hashlib.file_digest(handle, "sha256").hexdigest()
    if digest != LINUXDEPLOY_SHA256.get(architecture):
        raise ValueError("linuxdeploy checksum/architecture mismatch; tool was not executed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--linuxdeploy", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/feishin-desktop")
    parser.add_argument("--out", type=Path, default=ROOT / "target/appimage")
    args = parser.parse_args()
    architecture = platform.machine()
    tool = args.linuxdeploy.resolve()
    verify_tool(tool, architecture)
    libraries = runtime_libraries(subprocess.check_output(["ldconfig", "-p"], text=True), architecture)
    library_args = [argument for path in libraries for argument in ("--library", path)]
    args.out.mkdir(parents=True, exist_ok=True)
    output = args.out.resolve() / f"Feishin-Rust-P01-{architecture}.AppImage"
    if output.exists():
        raise SystemExit(f"refusing to overwrite existing artifact: {output}")
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))
    with tempfile.TemporaryDirectory(prefix="p01-appimage-", dir=args.out.resolve()) as temporary:
        temporary = Path(temporary)
        appdir = temporary / "AppDir"
        notices = appdir / "usr/share/doc/feishin-p01"
        collect(metadata, notices)
        (notices / "PACKAGING-SCOPE.txt").write_text(
            "P01 development smoke artifact, not a release. Requires the build distribution's glibc baseline and system GPU/display drivers. "
            "mpv and FFmpeg are external optional probe prerequisites; neither is bundled. No product login/playback or profile migration exists. "
            "Linux x86_64/aarch64 clean startup, resources, audio and complete native-library/runtime licensing still require P01/P13 evidence.\n")
        desktop = temporary / "feishin-p01.desktop"
        desktop.write_text("[Desktop Entry]\nType=Application\nName=Feishin Rust P01\nExec=feishin-desktop\nIcon=feishin-p01\nCategories=AudioVideo;Audio;\nTerminal=false\n")
        icon = temporary / "feishin-p01.svg"
        shutil.copyfile(ROOT / "tools/app/assets/feishin-p01.svg", icon)
        env = os.environ.copy()
        env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
        env["ARCH"] = architecture
        subprocess.run([str(tool), "--appimage-extract-and-run", "--appdir", str(appdir),
                        "--executable", str(args.binary.resolve()), "--desktop-file", str(desktop),
                        "--icon-file", str(icon), *library_args, "--output", "appimage"], cwd=temporary, env=env, check=True)
        artifacts = list(temporary.glob("*.AppImage"))
        if len(artifacts) != 1:
            raise SystemExit("expected exactly one generated AppImage")
        shutil.copyfile(artifacts[0], output)
        output.chmod(0o755)
    print(output)


if __name__ == "__main__":
    main()
