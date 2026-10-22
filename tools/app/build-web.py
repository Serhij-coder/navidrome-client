#!/usr/bin/env python3
"""Build separate full-web and remote WASM feasibility bundles from tracked inputs."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
from notices import ROOT, collect


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "target/p01-web")
    args = parser.parse_args()
    version = subprocess.check_output(["wasm-bindgen", "--version"], text=True).strip()
    if version != "wasm-bindgen 0.2.106":
        raise SystemExit("wasm-bindgen-cli 0.2.106 required; see docs/app/p01/README.md")
    subprocess.run(["cargo", "build", "--locked", "--release", "--target", "wasm32-unknown-unknown", "-p", "feishin-web", "-p", "feishin-remote"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))
    target = Path(metadata["target_directory"]) / "wasm32-unknown-unknown/release"
    for app in ("web", "remote"):
        output = args.out.resolve() / app
        output.mkdir(parents=True, exist_ok=True)
        subprocess.run(["wasm-bindgen", "--target", "web", "--out-dir", str(output), "--out-name", "app", str(target / f"feishin_{app}.wasm")], check=True)
        shutil.copyfile(ROOT / "tools/app/browser-shell.html", output / "index.html")
        collect(metadata, output / "notices")
    print(f"Serve {args.out.resolve()} on loopback; routes /web/ and /remote/. No deployment performed.")


if __name__ == "__main__":
    main()
