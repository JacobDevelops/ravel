"""Regression coverage for the real frozen WP0 evidence and phase boundary."""
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True

MODULE_PATH = Path(__file__).with_name("verify-wp0.py")
SPEC = importlib.util.spec_from_file_location("verify_wp0", MODULE_PATH)
WP0 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WP0)
ROOT = Path(__file__).resolve().parents[2]
ACQUISITION_SPEC = importlib.util.spec_from_file_location("artifact_set", MODULE_PATH.with_name("artifact-set.py"))
ACQUISITION = importlib.util.module_from_spec(ACQUISITION_SPEC)
ACQUISITION_SPEC.loader.exec_module(ACQUISITION)
REBIND_SPEC = importlib.util.spec_from_file_location("rebind_references", MODULE_PATH.with_name("rebind-references.py"))
REBIND = importlib.util.module_from_spec(REBIND_SPEC)
REBIND_SPEC.loader.exec_module(REBIND)


class WP0EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.workspace = tempfile.TemporaryDirectory()
        self.addCleanup(self.workspace.cleanup)
        self.root = Path(self.workspace.name)
        self.inventory = self.root / "registry/inventory" / WP0.FIXTURE
        shutil.copytree(ROOT / "registry/inventory" / WP0.FIXTURE, self.inventory)
        descriptors = self.root / "tests/fixtures/descriptors"
        descriptors.mkdir(parents=True)
        shutil.copyfile(ROOT / "tests/fixtures/descriptors" / (WP0.FIXTURE + ".json"),
                        descriptors / (WP0.FIXTURE + ".json"))
        checks = self.root / "registry/checks"
        checks.mkdir()
        shutil.copyfile(ROOT / "registry/checks/public-word-collisions.tsv", checks / "public-word-collisions.tsv")
        selected = self.root / "registry/sevtech"
        selected.mkdir()
        (selected / "public-word-collisions.tsv").write_text("")
        shutil.copyfile(ROOT / "registry/sevtech/evidence-pins.json", selected / "evidence-pins.json")

    def mutate(self, name, change):
        path = self.inventory / (name + ".json")
        data = json.loads(path.read_text())
        change(data)
        path.write_text(json.dumps(data))

    def test_committed_evidence_validates_offline_without_local_inputs(self):
        WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_wrong_fixture_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "WP0 phase requires fixture"):
            WP0.validate(self.root, "atm10-tts-2.0.2", "--offline")

    def test_mismatched_committed_fixture_is_rejected(self):
        self.mutate("graph", lambda data: data["fixture"].update(id="atm10-tts-2.0.2"))
        with self.assertRaisesRegex(ValueError, "graph fixture identity"):
            WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_descriptor_hash_mismatch_is_rejected(self):
        self.mutate("references", lambda data: data["fixture"]["archive"].update(sha256="0" * 64))
        with self.assertRaisesRegex(ValueError, "archive descriptor hash"):
            WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_artifact_hash_mismatch_is_rejected(self):
        self.mutate("graph-evidence", lambda data: data["artifacts"][0].update(sha256="0" * 64))
        with self.assertRaisesRegex(ValueError, "graph evidence artifact hash"):
            WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_artifact_set_hash_mismatch_is_rejected(self):
        self.mutate("graph", lambda data: data["fixture"]["artifactSet"].update(sha256="0" * 64))
        with self.assertRaisesRegex(ValueError, "graph artifact-set binding"):
            WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_index_aligned_edge_mismatch_is_rejected(self):
        self.mutate("graph-evidence", lambda data: data["edges"][0].update(to="invented"))
        with self.assertRaisesRegex(ValueError, "edge evidence alignment"):
            WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_coordinated_edge_edit_is_rejected_by_frozen_pins(self):
        for name in ("graph", "graph-evidence"):
            self.mutate(name, lambda data: data["edges"][0].update(versionRange="changed"))
        with self.assertRaisesRegex(ValueError, "frozen evidence bytes differ"):
            WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_frozen_exclusion_count_cannot_be_changed(self):
        path = self.root / "registry/sevtech/evidence-pins.json"
        data = json.loads(path.read_text())
        data["excludedDeclarations"] = 4
        path.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "frozen excluded declaration count"):
            WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_rebind_refuses_changed_references_even_if_structurally_equivalent(self):
        path = self.inventory / "references.json"
        path.write_bytes(path.read_bytes() + b"\n")
        before = (self.inventory / "graph-evidence.json").read_bytes()
        with self.assertRaisesRegex(ValueError, "references bytes differ from approved frozen input"):
            REBIND.rebind(self.root)
        self.assertEqual((self.inventory / "graph-evidence.json").read_bytes(), before)

    def test_missing_node_is_rejected(self):
        self.mutate("graph", lambda data: data["nodes"].pop())
        with self.assertRaisesRegex(ValueError, "declared node identities"):
            WP0.validate(self.root, WP0.FIXTURE, "--offline")

    def test_wrong_phase_is_rejected_before_registry_checks(self):
        result = subprocess.run(["bash", str(MODULE_PATH.with_name("verify.sh")),
                                 "--offline", "--phase", "invented"], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertIn("unknown registry phase", result.stderr)

    def test_wp0_phase_refuses_atm10(self):
        import os
        result = subprocess.run(["bash", str(MODULE_PATH.with_name("verify.sh")), "--phase", "wp0"],
                                env={**os.environ, "RAVEL_FIXTURE_ID": "atm10-tts-2.0.2"},
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertIn("WP0 phase requires", result.stderr)

    def test_full_sevtech_refuses_named_missing_obligations(self):
        import os
        result = subprocess.run(["bash", str(MODULE_PATH.with_name("verify.sh")),
                                 "--offline", "--phase", "full"],
                                env={**os.environ, "RAVEL_FIXTURE_ID": WP0.FIXTURE},
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1)
        for obligation in ("reproducible legacy graph derivation (mcmod.info / @Mod extraction)",
                           "surfaces inventory", "implementation plans", "observed reads reconciliation"):
            self.assertIn("NOT-YET-MET: " + obligation, result.stderr)


    def test_new_graph_identity_is_refused_in_public_content(self):
        (self.root / "public.txt").write_text("A SYNTHETIC_MOD declaration.")
        with self.assertRaisesRegex(ValueError, "without recorded collision decision"):
            WP0.validate_public_boundary(self.root, ["synthetic_mod"], ["public.txt"])

    def test_new_graph_identity_is_refused_in_public_path(self):
        (self.root / "synthetic_mod.txt").write_text("neutral")
        with self.assertRaisesRegex(ValueError, "without recorded collision decision"):
            WP0.validate_public_boundary(self.root, ["synthetic_mod"], ["synthetic_mod.txt"])

    def test_registered_decision_allows_observed_word_without_requiring_other_decisions(self):
        (self.root / "public.txt").write_text("Create a value.")
        WP0.validate_public_boundary(self.root, ["create"], ["public.txt"])

    def test_selected_decisions_allow_observed_inventory_words(self):
        path = self.root / "registry/sevtech/public-word-collisions.tsv"
        shutil.copyfile(ROOT / "registry/sevtech/public-word-collisions.tsv", path)
        identities = {line.split("\t", 1)[0] for line in path.read_text().splitlines()}
        (self.root / "public.txt").write_text(" ".join(sorted(identities)))
        WP0.validate_public_boundary(self.root, identities, ["public.txt"])

    def test_selected_decision_requires_inventory_identity(self):
        (self.root / "registry/sevtech/public-word-collisions.tsv").write_text("synthetic_mod\tGeneric test vocabulary.\n")
        (self.root / "public.txt").write_text("neutral")
        with self.assertRaisesRegex(ValueError, "not a graph identity"):
            WP0.validate_public_boundary(self.root, ["another_mod"], ["public.txt"])

    def test_selected_decision_requires_observed_occurrence(self):
        (self.root / "registry/sevtech/public-word-collisions.tsv").write_text("synthetic_mod\tGeneric test vocabulary.\n")
        (self.root / "public.txt").write_text("neutral")
        with self.assertRaisesRegex(ValueError, "no public occurrence"):
            WP0.validate_public_boundary(self.root, ["synthetic_mod"], ["public.txt"])

    def test_acquisition_manifest_required_and_lock_fields_are_bound(self):
        references = [{"projectId": 1, "fileId": 2, "required": True, "locked": None}]
        row = {"projectID": 1, "fileID": 2, "required": True}
        ACQUISITION.validate_manifest_references({"files": [row]}, references)
        for field, value in (("required", False), ("isLocked", True)):
            with self.subTest(field=field), self.assertRaisesRegex(SystemExit, "required or isLocked differs"):
                ACQUISITION.validate_manifest_references({"files": [{**row, field: value}]}, references)

    def test_registry_is_excluded_and_substrings_are_not_words(self):
        (self.root / "public.txt").write_text("synthetic_mod_suffix")
        (self.root / "registry/private.txt").write_text("synthetic_mod")
        WP0.validate_public_boundary(self.root, ["synthetic_mod"], ["public.txt", "registry/private.txt"])


if __name__ == "__main__":
    unittest.main()
