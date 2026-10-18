"""Direct regression coverage for byte-bound and offline WP0 rights helpers."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
from typing import Any
import unittest
from unittest.mock import patch
import zipfile

sys.dont_write_bytecode = True

EVIDENCE_SPEC = importlib.util.spec_from_file_location(
    "rights_evidence", Path(__file__).with_name("rights-evidence.py"))
EVIDENCE = importlib.util.module_from_spec(EVIDENCE_SPEC)
EVIDENCE_SPEC.loader.exec_module(EVIDENCE)
STATIC_SPEC = importlib.util.spec_from_file_location(
    "rights_static", Path(__file__).with_name("rights-static.py"))
STATIC = importlib.util.module_from_spec(STATIC_SPEC)
STATIC_SPEC.loader.exec_module(STATIC)
ROOT = Path(__file__).resolve().parents[2]


class RightsHelpersTests(unittest.TestCase):
    def setUp(self):
        workspace = tempfile.TemporaryDirectory()
        self.addCleanup(workspace.cleanup)
        self.root = Path(workspace.name)

    def cache_json(self, value):
        data = json.dumps(value).encode()
        digest = hashlib.sha256(data).hexdigest()
        (self.root / digest).write_bytes(data)
        return digest

    def derived(self, family: str = "permissive", reserved: str = "false",
                branding: bool = False) -> dict[str, Any]:
        original = {"artifacts": [{"artifact": "example.jar", "rights": [
            {"surface": surface, "status": "unresolved"} for surface in STATIC.SURFACES
        ]}]}
        evidence = {"artifacts": {"node": {"artifact": "example.jar", "surfaceEvidence": {
            "counts": {"textures": 1, "models": 0, "sounds": 1}
        }}}}
        result = EVIDENCE.derived_rights(
            original, [f"example.jar\tMIT\t{family}\t{reserved}"],
            ["example.jar\tautomated-curseforge"], evidence, {"example.jar": branding})
        self.assertEqual(original["artifacts"][0]["rights"][0]["status"], "unresolved")
        return result

    def test_grant_matcher_requires_license_text_and_preserves_restrictions(self):
        mit = "Permission is hereby granted, free of charge, without restriction."
        for text, declared, expected in (
            (mit, "MIT", True), ("MIT", "MIT", False),
            (mit + " Non-commercial use only", "MIT", False),
            (mit + " Assets all rights reserved", "MIT", False),
            (mit + " Assets all rights reserved", "MIT; assets reserved", True),
            ("GNU Lesser General Public License Version 2.1", "LGPL-2.1", True),
            ("GNU Lesser General Public License Version 2.1", "GPL-2.1", False),
            ("Creative Commons Attribution NonCommercial ShareAlike 4.0", "CC-BY-SA-4.0", False),
            ("Creative Commons Attribution NonCommercial ShareAlike 4.0", "CC-BY-NC-SA-4.0", True),
        ):
            with self.subTest(text=text, declared=declared):
                self.assertEqual(EVIDENCE.grant_text_matches(text, declared), expected)

    def test_cached_grant_rejects_missing_and_modified_bytes(self):
        digest = self.cache_json({"license": "MIT"})
        self.assertEqual(json.loads(EVIDENCE.cached_bytes(self.root, digest)), {"license": "MIT"})
        (self.root / digest).write_bytes(b"changed")
        with self.assertRaisesRegex(SystemExit, "cached grant SHA256 differs"):
            EVIDENCE.cached_bytes(self.root, digest)
        with self.assertRaisesRegex(SystemExit, "missing byte-bound grant cache"):
            EVIDENCE.cached_bytes(self.root, "0" * 64)
        with self.assertRaisesRegex(SystemExit, "missing cached response SHA256"):
            EVIDENCE.cached_bytes(self.root, "not-a-digest")

    def test_exact_file_grant_binds_both_artifact_hashes_and_project(self):
        artifact = b"acquired artifact bytes"
        sha1 = hashlib.sha1(artifact).hexdigest()
        citation = {"api": "https://api.modrinth.com/v2/version_files",
                    "project": "example", "licenseId": "MIT"}
        version = {sha1: {"project_id": "project-id", "files": [{"hashes": {
            "sha1": sha1, "sha512": hashlib.sha512(artifact).hexdigest()
        }}]}}
        receipt = {"fetchedAt": "2026-01-01T00:00:00Z", "endpoint": citation["api"],
                   "hashAlgorithm": "sha1", "artifactHash": sha1,
                   "versionResponseSha256": self.cache_json(version),
                   "projectEndpoint": "https://api.modrinth.com/v2/project/project-id",
                   "projectResponseSha256": self.cache_json({
                       "id": "project-id", "slug": "example", "license": {"id": "MIT"}})}
        receipt_path = self.root / ("modrinth-" + sha1 + ".json")
        receipt_path.write_text(json.dumps(receipt))
        self.assertEqual(EVIDENCE.exact_file_grant(self.root, artifact, citation, "MIT"), receipt)
        with self.assertRaisesRegex(SystemExit, "does not authorize"):
            EVIDENCE.exact_file_grant(self.root, artifact, citation, "Apache-2.0")
        version[sha1]["files"][0]["hashes"]["sha512"] = "0" * 128
        receipt["versionResponseSha256"] = self.cache_json(version)
        receipt_path.write_text(json.dumps(receipt))
        with self.assertRaisesRegex(SystemExit, "does not identify acquired jar bytes"):
            EVIDENCE.exact_file_grant(self.root, artifact, citation, "MIT")

    def test_repository_link_requires_observed_repository_not_redirect_claim(self):
        repository = {"url": "https://github.com/owner/project"}
        self.assertTrue(EVIDENCE.repository_linked(repository, ["https://GitHub.com/OWNER/PROJECT/issues"]))
        self.assertFalse(EVIDENCE.repository_linked(repository, ["https://github.com/owner/project-other"]))
        self.assertFalse(EVIDENCE.repository_linked(repository, ["https://github.com/previous/project"]))

    def test_derived_rights_and_static_gate_agree_for_each_code_policy(self):
        for family, code in (("permissive", "cleared"),
                             ("attribution-required", "permission-required"),
                             ("absent", "unresolved"), ("all-rights-reserved", "prohibited")):
            with self.subTest(family=family):
                document = self.derived(family)
                row = document["artifacts"][0]
                rights = {r["surface"]: r["status"] for r in row["rights"]}
                self.assertEqual(rights["code"], code)
                self.assertEqual(rights["textures"], code)
                self.assertEqual(rights["models"], "not-applicable")
                self.assertEqual(row["publication"], "blocked:" + (
                    "prohibited" if code == "prohibited" else "unresolved"))
                self.assertEqual(row["sourceUse"], STATIC.SOURCE_USE[code])
                STATIC.validate_derived(row, rights, code, "false", "example.jar")
                STATIC.validate_totals(document)

    def test_reserved_assets_and_branding_override_permissive_code(self):
        document = self.derived(reserved="true", branding=True)
        row = document["artifacts"][0]
        rights = {r["surface"]: r["status"] for r in row["rights"]}
        self.assertEqual(rights["code"], "cleared")
        self.assertEqual(rights["textures"], "prohibited")
        self.assertEqual(rights["branding"], "prohibited")
        self.assertEqual(rights["models"], "not-applicable")
        self.assertEqual(row["publication"], "blocked:prohibited")
        STATIC.validate_derived(row, rights, "cleared", "true", "example.jar")

    def test_static_totals_check_reads_every_aggregate_key(self):
        document = self.derived()
        STATIC.validate_totals(document)
        for key in document["totals"]:
            with self.subTest(key=key):
                changed = copy.deepcopy(document)
                changed["totals"][key] = None
                with self.assertRaisesRegex(SystemExit, "totals differ"):
                    STATIC.validate_totals(changed)

    def test_static_record_refuses_duplicate_surfaces_and_false_publication(self):
        row = self.derived()["artifacts"][0]
        row.update({"class": "mod", "reference": {"projectId": 1, "fileId": 2}})
        record = {"projectId": 1, "fileId": 2}
        arguments = (record, set(STATIC.RANK), {"permissive": "cleared"},
                     {"MIT": ("permissive", "false")})
        STATIC.validate_record(row, *arguments)
        duplicate = copy.deepcopy(row)
        duplicate["rights"][-1] = copy.deepcopy(duplicate["rights"][0])
        with self.assertRaisesRegex(SystemExit, "every surface exactly once"):
            STATIC.validate_record(duplicate, *arguments)
        row["publication"] = "cleared"
        with self.assertRaisesRegex(SystemExit, "publication differs"):
            STATIC.validate_record(row, *arguments)

    def test_failed_optional_matcher_retry_keeps_unresolved_policy_and_trail(self):
        resolution = {"status": "unresolved", "declared": "MIT", "basis": "licence-file",
                      "trail": [{"result": "WP7 correction: cited legal text does not state the recorded grant:"}]}
        original = copy.deepcopy(resolution)
        with patch.object(EVIDENCE, "repository_grant", side_effect=SystemExit("invalid grant")) as repository_grant:
            result = EVIDENCE.resolve_grant(
                "example.jar", {"hostDeclaration": {"declared": ""}, "resolution": resolution},
                [], b"", self.root, {("", "absent", "false")}, True)
        repository_grant.assert_called_once_with("example.jar", resolution, [], self.root, True)
        self.assertEqual(result, ("", "absent", "false"))
        self.assertEqual(resolution, original)

    def test_evidence_entrypoint_observes_jar_and_serializes_sorted_decisions(self):
        metadata = json.dumps([{"modid": "example", "license": "MIT"}]).encode()
        jar = self.root / "example.jar"
        with zipfile.ZipFile(jar, "w") as archive:
            archive.writestr("mcmod.info", metadata)
        evidence = {"artifacts": {"node": {"artifact": jar.name, "nodeIds": ["node"],
                    "artifactSha256": hashlib.sha256(jar.read_bytes()).hexdigest(),
                    "acquisitionStatus": "fetched", "hostDeclaration": {"declared": "MIT", "evidence": [
                        {"kind": "jar-entry", "source": jar.name + "!mcmod.info",
                         "sha256": hashlib.sha256(metadata).hexdigest()}
                    ]}}}}
        evidence_path = self.root / "evidence.json"
        evidence_path.write_text(json.dumps(evidence))
        paths = self.root / "paths.tsv"
        paths.write_text(jar.name + "\t" + str(jar) + "\n")
        policies = self.root / "policies.tsv"
        policies.write_text("MIT\tpermissive\tfalse\n")
        grants, acquisition = self.root / "grants.tsv", self.root / "acquisition.tsv"
        argv = ["rights-evidence.py", str(evidence_path), str(paths), str(policies),
                str(grants), str(acquisition), str(self.root)]
        with patch.object(sys, "argv", argv):
            EVIDENCE.main()
        self.assertEqual(grants.read_text(), "example.jar\tMIT\tpermissive\tfalse\n")
        self.assertEqual(acquisition.read_text(), "example.jar\tautomated-curseforge\n")
        jar.write_bytes(b"changed")
        with patch.object(sys, "argv", argv), self.assertRaisesRegex(SystemExit, "artifact SHA256 differs"):
            EVIDENCE.main()

    def test_static_entrypoint_validates_recorded_fixture_without_local_bytes(self):
        output = io.StringIO()
        with patch.object(sys, "argv", ["rights-static.py", str(ROOT), "sevtech-ages-3.2.3"]):
            with contextlib.redirect_stdout(output):
                STATIC.main()
        self.assertIn("records match vocabulary, policy, derived statuses and totals", output.getvalue())


if __name__ == "__main__":
    unittest.main()
