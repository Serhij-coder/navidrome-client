import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("p00_check", ROOT / "tools/app/check-p00.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class EvidenceChecks(unittest.TestCase):
    def test_csv_carriage_returns_are_rejected_before_staging(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "record.csv"
            path.write_bytes(b"feature_id,value\r\nF28,proposal\r\n")
            with self.assertRaisesRegex(ValueError, "LF line endings"):
                MODULE.csv_rows(path)

    def test_current_tracked_evidence(self):
        self.assertEqual([], MODULE.validate(ROOT))

    def test_reject_false_route_and_position_claims(self):
        original = json.loads((ROOT / "docs/app/baseline/source-audit.json").read_text())
        changed = copy.deepcopy(original)
        changed["routes"][0]["registrations"] = []
        self.assertIn("route declaration/registration evidence changed", MODULE.validate_audit(changed))
        changed = copy.deepcopy(original)
        changed["characterization"]["position"]["result"][2] = 180000
        self.assertIn("position unit characterization changed", MODULE.validate_audit(changed))

    def test_target_improvements_cannot_be_labeled_original_expectations(self):
        baseline = ROOT / "docs/app/baseline"
        row = next(r for r in MODULE.csv_rows(baseline / "journeys.csv") if r["feature_id"] == "F28")
        self.assertEqual([], MODULE.validate_journey(row, baseline))
        row["evidence_kind"] = "source-derived"
        row["expected"] = row.pop("target_expected")
        errors = MODULE.validate_journey(row, baseline)
        self.assertIn("invalid/unsubstantiated journey: F28", errors)
        self.assertIn("journey must separate target acceptance and link decision: F28", errors)


if __name__ == "__main__":
    unittest.main()
