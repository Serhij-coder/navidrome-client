#!/usr/bin/env python3
"""Check tracked P00 evidence; reference archive/catalog checks are optional."""

import argparse
import csv
import hashlib
import json
from pathlib import Path
import re
import tarfile


COMMIT = "7d6ad318c79802e226996cfc39acba6b05bb22e8"
ARCHIVE_SHA = "a3986aa97191bf31695473ffbfd7982eb9c106ed4390b59cd52b1070213d6bcb"


def csv_rows(path):
    if b"\r" in path.read_bytes():
        raise ValueError(f"maintained CSV requires LF line endings: {path}")
    with path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def validate_audit(audit):
    errors = []
    if audit.get("commit") != COMMIT or audit.get("parser") != "6.0.3":
        errors.append("source audit provenance mismatch")
    routes = audit["routes"]
    unused = {r["name"] for r in routes if not r["registrations"]}
    if len(routes) != 33 or unused != {"EXPLORE", "PLAYING", "SERVERS"}:
        errors.append("route declaration/registration evidence changed")
    if len({r["name"] for r in routes}) != len(routes):
        errors.append("duplicate route name")
    if audit["graph"]["unresolved"]:
        errors.append("unresolved literal source imports")
    empty, active = audit["compositions"]
    if empty["bytes"] != 0 or empty["incoming"] or empty["roots"] or not active["roots"]:
        errors.append("player composition evidence mismatch")
    cases = audit["characterization"]
    if cases["position"]["result"] != [180000, 1800000000, 180000000, 1800000000000]:
        errors.append("position unit characterization changed")
    if cases["threshold"]["result"] != [True, True]:
        errors.append("BOTH threshold characterization changed")
    if cases["settingsWrite"]["result"] != {"general": None, "version": "14", "read": None}:
        errors.append("settings write characterization changed")
    return errors


def validate_journey(row, baseline):
    errors = []
    if not all(row.values()) or row["runtime_status"] != "not-run" or row["evidence_kind"] != "source-anchored-target-proposal" or row["reference_outcome"] != "not-captured":
        errors.append(f"invalid/unsubstantiated journey: {row['feature_id']}")
    if "expected" in row or not row.get("target_expected") or not (baseline / row["decision_refs"]).is_file():
        errors.append(f"journey must separate target acceptance and link decision: {row['feature_id']}")
    source, line, _ = row["upstream_id"].rsplit(":", 2)
    expected_url = f"https://github.com/jeffvli/feishin/blob/{COMMIT}/{source}#L{line}"
    if row["source_url"] != expected_url:
        errors.append(f"journey pinned link mismatch: {row['feature_id']}")
    return errors


def validate(root, archive=None, functions=None):
    baseline = root / "docs/app/baseline"
    audit = json.loads((baseline / "source-audit.json").read_text())
    errors = validate_audit(audit)
    journeys = csv_rows(baseline / "journeys.csv")
    feature_ids = {f"F{i:02}" for i in range(1, 53)}
    if len(journeys) != 52 or {r["feature_id"] for r in journeys} != feature_ids:
        errors.append("journeys must account for each F01-F52 exactly once")
    for row in journeys:
        errors.extend(validate_journey(row, baseline))
    migration = csv_rows(root / "docs/app/migration/p00-map.csv")
    if len(migration) != len(journeys) or [(r["feature_id"], r["upstream_id"]) for r in migration] != [(r["feature_id"], r["upstream_id"]) for r in journeys]:
        errors.append("migration anchors disagree with journeys")
    if any(r["disposition"] != "unmapped" or r["rust_symbol"] or r["status"] != "source-anchored-proposal" for r in migration):
        errors.append("P00 source anchors cannot claim a Rust migration")
    wire = json.loads((baseline / "wire-results.json").read_text())
    corpus = json.loads((root / "tests/fixtures/p00/songs.json").read_text())
    expected_ids = [s["id"] for s in corpus["songs"] + corpus["invalidSongs"]]
    if [s["id"] for s in wire["songs"]] != expected_ids:
        errors.append("wire results do not match fixture IDs")
    for sample, result in zip(corpus["songs"], wire["songs"]):
        expected = {"id": sample["expected"]["id"], "serverId": sample["server_id"], "durationMs": sample["expected"]["duration_ms"]}
        if result.get("normalized") != expected or result["schema"] != "passed":
            errors.append(f"normalization result mismatch: {sample['id']}")
    if any(r["schema"] != "rejected-as-expected" for r in wire["songs"][len(corpus["songs"]):]):
        errors.append("malformed fixtures were not rejected")
    scenarios = json.loads((root / "tests/fixtures/p00/server-scenarios.json").read_text())
    if len(scenarios["profiles"]) != 12 or scenarios["live_captures"]:
        errors.append("server scenario provenance mismatch")
    scratchpad = (root / "docs/app/STATUS.md").read_text()
    if len([line for line in scratchpad.splitlines() if line.strip()]) > 25:
        errors.append("STATUS.md exceeds 25 non-empty lines")
    markdown = [root / "README.md", root / "AGENTS.md", root / "THIRD_PARTY_NOTICES.md"]
    for directory in ["docs/app", "tools/app", "tests"]:
        markdown.extend((root / directory).rglob("*.md"))
    for file in markdown:
        for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", file.read_text()):
            target = target.strip("<>").split("#", 1)[0]
            if not target or re.match(r"[a-zA-Z][a-zA-Z0-9+.-]*:", target):
                continue
            if not (file.parent / target).exists():
                errors.append(f"broken local link: {file.relative_to(root)} -> {target}")
    if archive:
        if hashlib.sha256(archive.read_bytes()).hexdigest() != ARCHIVE_SHA:
            errors.append("reference archive hash mismatch")
        with tarfile.open(archive) as source:
            for name, expected_hash in audit["sources"].items():
                member = source.extractfile(name)
                if member is None or hashlib.sha256(member.read()).hexdigest() != expected_hash:
                    errors.append(f"source hash mismatch: {name}")
            for asset in csv_rows(baseline / "assets.csv"):
                member = source.extractfile(asset["upstream_path"])
                if member is None or hashlib.sha256(member.read()).hexdigest() != asset["sha256"]:
                    errors.append(f"asset hash mismatch: {asset['upstream_path']}")
    if functions:
        ids = {json.loads(line)["id"] for line in functions.read_text().splitlines()}
        for row in journeys:
            if row["upstream_id"] not in ids:
                errors.append(f"unknown stable upstream ID: {row['upstream_id']}")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--functions", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    try:
        errors = validate(root, args.archive, args.functions)
    except (OSError, KeyError, ValueError, tarfile.TarError) as error:
        parser.exit(1, f"P00 check failed: {error}\n")
    if errors:
        parser.exit(1, "\n".join(errors) + "\n")
    print("P00 tracked evidence checks passed; runtime gates remain open.")


if __name__ == "__main__":
    main()
