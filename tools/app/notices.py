#!/usr/bin/env python3
"""Collect exact registry/upstream license texts and bundled egui font notices.

This is a reproducible notice inventory, not a legal certification or release gate.
Registry-overrides bind omitted package license files to exact upstream revisions.
"""
import argparse
import gzip
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def notice_files(directory):
    return sorted(path for path in directory.rglob("*") if path.is_file() and not path.is_symlink() and (
        any(word in path.name.lower() for word in ("license", "copying", "notice", "copyright"))
        or path.suffix == ".txt" and "font" in str(path.relative_to(directory)).lower()
    ))


def copy_notice(source, target):
    """Preserve exact third-party bytes; expand losslessly stored source notices."""
    if source.suffix == ".gz":
        target = target.with_suffix("")
        target.parent.mkdir(parents=True, exist_ok=True)
        with gzip.open(source, "rb") as reader, target.open("wb") as writer:
            shutil.copyfileobj(reader, writer)
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)


def collect(metadata, output, root=ROOT):
    output.mkdir(parents=True, exist_ok=True)
    overrides = json.loads((root / "docs/app/licenses/registry-overrides.json").read_text())
    inventory = []
    for package in metadata["packages"]:
        if not package["source"]:
            continue
        identity = f"{package['name']}@{package['version']}"
        directory = Path(package["manifest_path"]).parent
        texts = notice_files(directory)
        destination = output / identity
        destination.mkdir(exist_ok=True)
        for source in texts:
            relative = source.relative_to(directory)
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            copy_notice(source, target)
        extra = overrides.get(identity, {}).get("files", [])
        for source in extra:
            path = (root / source).resolve()
            if not path.is_relative_to((root / "docs/app/licenses").resolve()):
                raise ValueError(f"license override outside tracked licenses: {identity}")
            copy_notice(path, destination / path.name)
        if not texts and not extra:
            raise ValueError(f"missing package license evidence: {identity}")
        # Preserve attribution from published manifests/readmes as well as license texts.
        for source in [directory / "Cargo.toml", *directory.glob("README*")]:
            if source.is_file() and not source.is_symlink():
                shutil.copyfile(source, destination / source.name)
        inventory.append({"package": identity, "license": package.get("license"),
                          "repository": package.get("repository"), "source": package["source"],
                          "upstream_override": overrides.get(identity)})
    for filename in ("LICENSE", "THIRD_PARTY_NOTICES.md", "Cargo.lock"):
        shutil.copyfile(root / filename, output / filename)
    toolchain = root / "docs/app/licenses/toolchain"
    for source in toolchain.rglob("*"):
        if source.is_file():
            copy_notice(source, output / "rust-toolchain" / source.relative_to(toolchain))
    (output / "registry-inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    return inventory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))
    inventory = collect(metadata, args.output)
    print(f"Collected {len(inventory)} registry package notice sets (all lockfile targets, conservatively overinclusive)")


if __name__ == "__main__":
    main()
