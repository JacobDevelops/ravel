"""Offline static WP0 rights gate refuses each derived-field tamper."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "registry/sevtech/rights-static.py"
FIXTURE = "sevtech-ages-3.2.3"


class StaticRightsTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        paths = (
            "schemas/provenance.schema.json", "registry/checks/licence-families.tsv",
            "registry/checks/licence-status.tsv",
            f"registry/inventory/{FIXTURE}/licence-families.tsv",
            f"registry/inventory/{FIXTURE}/references.json",
            f"registry/inventory/{FIXTURE}/rights.json",
        )
        for rel in paths:
            (self.root / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / rel, self.root / rel)
        self.rights_path = self.root / f"registry/inventory/{FIXTURE}/rights.json"
        self.rights = json.loads(self.rights_path.read_text())

    def tearDown(self):
        self.tmp.cleanup()

    def run_static(self):
        self.rights_path.write_text(json.dumps(self.rights))
        return subprocess.run([sys.executable, "-B", str(SCRIPT), str(self.root), FIXTURE],
                              capture_output=True, text=True, check=False)

    def assert_refused(self, message):
        result = self.run_static()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(message, result.stderr)

    def surface(self, row, name):
        return next(r for r in row["rights"] if r["surface"] == name)

    def test_committed_rights_pass(self):
        result = self.run_static()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unknown_status_refused(self):
        self.surface(self.rights["artifacts"][0], "names")["status"] = "maybe"
        self.assert_refused("status outside the public vocabulary")

    def test_code_status_tamper_refused(self):
        row = next(r for r in self.rights["artifacts"] if r["licence"]["family"] != "permissive")
        self.surface(row, "code")["status"] = "cleared"
        self.assert_refused("code status differs")

    def test_unrecorded_policy_refused(self):
        self.rights["artifacts"][0]["licence"]["family"] = "permissive"
        self.assert_refused("licence tuple has no recorded policy")

    def test_reserved_asset_cleared_refused(self):
        row = self.rights["artifacts"][0]
        row["licence"].update(declared="MIT License (code) / All Rights Reserved (assets)",
                               family="permissive", assetsReservedSeparately=True)
        self.surface(row, "code")["status"] = "cleared"
        row["sourceUse"] = "permitted"
        self.surface(row, "textures")["status"] = "cleared"
        self.assert_refused("textures status differs")

    def test_source_use_tamper_refused(self):
        self.rights["artifacts"][0]["sourceUse"] = "permitted"
        self.assert_refused("sourceUse differs")

    def test_publication_tamper_refused(self):
        self.rights["artifacts"][0]["publication"] = "cleared"
        self.assert_refused("publication differs")

    def test_totals_tamper_refused(self):
        self.rights["totals"]["sourceUseProhibited"] += 1
        self.assert_refused("totals differ")

    def test_missing_record_refused(self):
        self.rights["artifacts"].pop()
        self.assert_refused("exactly once")

    def test_reference_tamper_refused(self):
        row = next(r for r in self.rights["artifacts"] if r["reference"])
        row["reference"]["fileId"] += 1
        self.assert_refused("differs from the ledger")

    def test_artifact_set_tamper_refused(self):
        self.rights["fixture"]["artifactSet"]["sha256"] = "0" * 64
        self.assert_refused("artifact-set binding")

    def test_duplicate_policy_across_tables_refused(self):
        path = self.root / f"registry/inventory/{FIXTURE}/licence-families.tsv"
        path.write_text(path.read_text() + "MIT\tcustom\tfalse\n")
        self.assert_refused("more than one policy")


if __name__ == "__main__":
    unittest.main()
