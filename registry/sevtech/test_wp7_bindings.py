#!/usr/bin/env python3
"""Dependency-free WP7 binding regressions; all mutations stay in temporary trees.

Run with: python3 -m unittest discover -s registry/sevtech -p 'test_wp7_*.py'
"""
import hashlib
import io
import json
from pathlib import Path
import shutil
import runpy
import subprocess
import sys
import tempfile
import unittest
import zipfile


ROOT = Path(__file__).resolve().parents[2]
CHECKS = ROOT / "registry/sevtech"


def sha(content, algorithm="sha256"):
    return hashlib.new(algorithm, content).hexdigest()


def jar_bytes():
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, "w") as archive:
        archive.writestr("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")
    return stream.getvalue()


class StandaloneArtifacts(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.fixture = "binding-test"
        self.local = self.root / "tests/fixtures/local" / self.fixture
        checks = self.root / "registry/checks"
        checks.mkdir(parents=True)
        helpers = self.root / "registry/sevtech"
        helpers.mkdir()
        for helper in ("artifact-set.sh", "artifact-set.py"):
            shutil.copyfile(CHECKS / helper, helpers / helper)
        # Only repository discovery is supplied by the miniature tree. The real
        # artifact enumerator and every byte/hash reconciliation run unchanged.
        (checks / "public-repo.sh").write_text(
            'repo_root="${WP7_TEST_ROOT}"\nregistry_root="$repo_root/registry"\nFIXTURE_ID=binding-test\n'
        )
        self.script = helpers / "artifact-set.sh"
        self.acquired = self.local / "jars/acquired.jar"
        self.bundled = self.local / "extracted/overrides/mods/bundled.jar"
        self.manifest = self.local / "extracted/manifest.json"
        self.archive = self.local / "source/binding-test.zip"
        for path in (self.acquired, self.bundled, self.manifest, self.archive):
            path.parent.mkdir(parents=True, exist_ok=True)
        content = jar_bytes()
        self.acquired.write_bytes(content)
        self.bundled.write_bytes(content)
        self.manifest.write_text(json.dumps({"files": [{"projectID": 17, "fileID": 23, "required": True, "isLocked": False}]}))
        with zipfile.ZipFile(self.archive, "w") as archive:
            archive.writestr("overrides/mods/bundled.jar", content)
        self.data = {
            "fixture": {"id": self.fixture, "archive": self.pin(self.archive), "manifest": self.pin(self.manifest)},
            "references": [{"projectId": 17, "fileId": 23, "required": True, "locked": False}],
            "acquiredArtifacts": [dict(self.artifact(self.acquired), projectId=17, fileId=23)],
            "bundledArtifacts": [dict(self.artifact(self.bundled), archiveEntry="overrides/mods/bundled.jar")],
        }
        self.ledger = self.root / "registry/inventory/binding-test/references.json"
        self.ledger.parent.mkdir(parents=True)

    def pin(self, path):
        return {"path": str(path.relative_to(self.root)), "sha256": sha(path.read_bytes())}

    def artifact(self, path):
        content = path.read_bytes()
        return dict(self.pin(path), artifact=path.name, status="fetched", size=len(content), sha1=sha(content, "sha1"))

    def run_binding(self):
        self.ledger.write_text(json.dumps(self.data))
        import os
        environment = dict(os.environ, WP7_TEST_ROOT=str(self.root))
        return subprocess.run(
            ["bash", "-c", 'source "$1"; test "$acquisition_mode" = standalone; test -s "$artifact_paths_tsv"', "binding-test", str(self.script)],
            env=environment, capture_output=True, text=True, check=False,
        )

    def assert_refused(self):
        result = self.run_binding()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_complete_standalone_population_is_accepted(self):
        result = self.run_binding()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_archive_hash_mismatch_is_refused(self):
        self.data["fixture"]["archive"]["sha256"] = "0" * 64
        self.assert_refused()

    def test_manifest_hash_mismatch_is_refused(self):
        self.data["fixture"]["manifest"]["sha256"] = "0" * 64
        self.assert_refused()

    def test_missing_acquired_jar_is_refused(self):
        self.acquired.unlink()
        self.assert_refused()

    def test_missing_bundled_jar_is_refused(self):
        self.bundled.unlink()
        self.assert_refused()

    def test_unrecorded_jar_is_refused(self):
        (self.acquired.parent / "extra.jar").write_bytes(jar_bytes())
        self.assert_refused()

    def test_reference_pair_mismatch_is_refused(self):
        self.data["acquiredArtifacts"][0]["fileId"] = 99
        self.assert_refused()


class RightsEvidence(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.cache = self.root / "grants"
        self.cache.mkdir()
        self.jar = self.root / "example.jar"
        self.jar.write_bytes(jar_bytes())
        self.paths = self.root / "paths.tsv"
        self.paths.write_text(f"example.jar\t{self.jar}\n")
        self.policies = self.root / "families.tsv"
        self.policies.write_text("\tabsent\tfalse\nMIT\tpermissive\tfalse\n")
        self.record = {
            "artifact": "example.jar", "nodeIds": ["example"],
            "artifactSha256": sha(self.jar.read_bytes()), "acquisitionStatus": "fetched",
            "hostDeclaration": {"declared": "", "evidence": [{
                "kind": "jar-entry", "source": "example.jar!META-INF/MANIFEST.MF",
                "sha256": sha(b"Manifest-Version: 1.0\n"),
            }]},
        }
        self.evidence = {"artifacts": {"example": self.record}}

    def run_binding(self):
        evidence = self.root / "evidence.json"
        evidence.write_text(json.dumps(self.evidence))
        return subprocess.run(
            [sys.executable, str(CHECKS / "rights-evidence.py"), str(evidence), str(self.paths),
             str(self.policies), str(self.root / "grants.tsv"), str(self.root / "acquisition.tsv"), str(self.cache)],
            capture_output=True, text=True, check=False,
        )

    def assert_no_permissive_grant(self, message):
        result = self.run_binding()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(message, result.stderr)
        grants = self.root / "grants.tsv"
        self.assertTrue(
            result.returncode != 0 or not grants.exists() or "\tpermissive\t" not in grants.read_text(),
            "unbound citation granted code permission: " + result.stdout + result.stderr,
        )

    def add_unrelated_resolution(self):
        # Syntactically valid citations are not proof that this artifact's
        # publisher made the grant. No network or real inventory is involved.
        self.record["resolution"] = {
            "status": "resolved", "basis": "licence-file", "declared": "MIT",
            "family": "permissive", "assetsReservedSeparately": False,
            "repository": {
                "url": "https://github.com/unrelated/project", "commit": "a" * 40,
                "licenceFile": {"path": "LICENSE", "sha256": sha(b"unrelated MIT licence"),
                    "url": "https://github.com/unrelated/project/blob/" + "a" * 40 + "/LICENSE"},
            },
        }
        content = b"MIT License\nPermission is hereby granted, free of charge, to deal in the Software without restriction."
        self.record["resolution"]["repository"]["licenceFile"]["sha256"] = sha(content)
        self.record["resolution"]["repository"]["licenceFile"]["blobSha"] = hashlib.sha1(b"blob " + str(len(content)).encode() + b"\0" + content).hexdigest()
        (self.cache / sha(content)).write_bytes(content)
        self.add_commit_observation()

    def add_commit_observation(self):
        repository = self.record["resolution"]["repository"]
        citation = repository["licenceFile"]
        endpoint = "https://api.github.com/repos/" + repository["url"].removeprefix("https://github.com/") + "/contents/" + citation["path"] + "?ref=" + repository["commit"]
        response = json.dumps({"type": "file", "path": citation["path"], "sha": citation["blobSha"]}).encode()
        (self.cache / sha(response)).write_bytes(response)
        observation = {"fetchedAt": "2026-10-19T06:05:25Z", "endpoint": endpoint, "responseSha256": sha(response)}
        self.commit_receipt_path = self.cache / ("github-contents-" + sha(endpoint.encode()) + ".json")
        self.commit_receipt_path.write_text(json.dumps(observation))
        citation["observation"] = observation

    def add_metadata(self, url, licence=""):
        content = json.dumps([{"modid": "example", "url": url, "license": licence}]).encode()
        with zipfile.ZipFile(self.jar, "w") as archive:
            archive.writestr("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")
            archive.writestr("mcmod.info", content)
        self.record["artifactSha256"] = sha(self.jar.read_bytes())
        self.record["hostDeclaration"]["evidence"].append({
            "kind": "jar-entry", "source": "example.jar!mcmod.info", "sha256": sha(content),
        })

    def add_exact_file_grant(self):
        artifact = self.jar.read_bytes()
        sha1 = hashlib.sha1(artifact).hexdigest()
        version = {sha1: {"project_id": "project-id", "files": [{"hashes": {
            "sha1": sha1, "sha512": hashlib.sha512(artifact).hexdigest(),
        }}]}}
        project = {"id": "project-id", "slug": "example", "license": {"id": "MIT"}}
        receipt = {
            "fetchedAt": "2026-10-18T06:05:25+00:00",
            "endpoint": "https://api.modrinth.com/v2/version_files",
            "hashAlgorithm": "sha1", "artifactHash": sha1,
            "projectEndpoint": "https://api.modrinth.com/v2/project/example",
        }
        for field, document in (("versionResponseSha256", version), ("projectResponseSha256", project)):
            content = json.dumps(document).encode()
            receipt[field] = sha(content)
            (self.cache / sha(content)).write_bytes(content)
        self.receipt_path = self.cache / ("modrinth-" + sha1 + ".json")
        self.receipt_path.write_text(json.dumps(receipt))
        self.record["resolution"] = {
            "status": "resolved", "basis": "modrinth-exact-file", "declared": "MIT",
            "family": "permissive", "assetsReservedSeparately": False,
            "modrinth": {
                "api": receipt["endpoint"], "hashAlgorithm": "sha1",
                "project": "example", "licenseId": "MIT", "observation": receipt,
            },
        }

    def test_exact_file_and_project_observation_permit_code(self):
        self.add_exact_file_grant()
        result = self.run_binding()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("\tMIT\tpermissive\tfalse", (self.root / "grants.tsv").read_text())

    def test_changed_exact_file_receipt_is_refused(self):
        self.add_exact_file_grant()
        receipt = json.loads(self.receipt_path.read_text())
        receipt["fetchedAt"] = "2026-10-19T06:05:25+00:00"
        self.receipt_path.write_text(json.dumps(receipt))
        self.assert_no_permissive_grant("observation receipt differs")

    def test_wrong_exact_file_request_hash_is_refused(self):
        self.add_exact_file_grant()
        receipt = json.loads(self.receipt_path.read_text())
        receipt["artifactHash"] = "0" * 40
        self.receipt_path.write_text(json.dumps(receipt))
        self.assert_no_permissive_grant("observation request identity differs")

    def test_wrong_exact_file_raw_response_hash_is_refused(self):
        self.add_exact_file_grant()
        receipt = self.record["resolution"]["modrinth"]["observation"]
        (self.cache / receipt["versionResponseSha256"]).write_bytes(b"{}")
        self.assert_no_permissive_grant("cached grant SHA256 differs")

    def test_exact_project_licence_cannot_be_relabelled(self):
        self.add_exact_file_grant()
        citation = self.record["resolution"]["modrinth"]
        project = {"id": "project-id", "slug": "example", "license": {"id": "LicenseRef-All-Rights-Reserved"}}
        content = json.dumps(project).encode()
        (self.cache / sha(content)).write_bytes(content)
        citation["licenseId"] = project["license"]["id"]
        citation["observation"]["projectResponseSha256"] = sha(content)
        self.receipt_path.write_text(json.dumps(citation["observation"]))
        self.assert_no_permissive_grant("does not authorize the recorded declaration")

    def test_duplicate_policy_declaration_is_refused(self):
        self.policies.write_text(self.policies.read_text() + "MIT\tpermissive\tfalse\n")
        self.assert_no_permissive_grant("duplicate licence policy declaration")

    def test_wrong_git_blob_identity_is_refused(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project")
        self.record["resolution"]["repository"]["licenceFile"]["blobSha"] = "0" * 40
        self.assert_no_permissive_grant("cited Git blob identity differs")

    def test_cached_non_mit_legal_text_cannot_be_labelled_mit(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project")
        content = b"All rights reserved; this is not an MIT permission grant."
        (self.cache / sha(content)).write_bytes(content)
        self.record["resolution"]["repository"]["licenceFile"]["sha256"] = sha(content)
        self.record["resolution"]["repository"]["licenceFile"]["blobSha"] = hashlib.sha1(b"blob " + str(len(content)).encode() + b"\0" + content).hexdigest()
        self.assert_no_permissive_grant("cited legal text does not state the recorded grant")

    def test_mit_substring_in_permitted_is_not_a_host_declaration(self):
        content = b"Limited use is permitted."
        with zipfile.ZipFile(self.jar, "a") as archive:
            archive.writestr("LICENSE", content)
        self.record["artifactSha256"] = sha(self.jar.read_bytes())
        self.record["hostDeclaration"]["declared"] = "MIT"
        self.record["hostDeclaration"]["evidence"].append({
            "kind": "jar-entry", "source": "example.jar!LICENSE",
            "sha256": sha(content), "role": "host-licence",
        })
        self.assert_no_permissive_grant("lacks independently observed declaration")

    def test_missing_git_blob_identity_is_refused(self):
        self.add_unrelated_resolution()
        del self.record["resolution"]["repository"]["licenceFile"]["blobSha"]
        self.assert_no_permissive_grant("missing pinned Git blob identity")

    def test_restrictive_grant_terms_cannot_be_dropped(self):
        matches = runpy.run_path(str(CHECKS / "rights-evidence.py"))["grant_text_matches"]
        mit = "Permission is hereby granted free of charge without restriction."
        cases = (
            ("CC-BY-4.0", "Creative Commons Attribution NonCommercial NoDerivatives 4.0"),
            ("CC-BY-4.0", "Creative Commons Attribution ShareAlike 4.0"),
            ("MIT", mit + " All Rights Reserved (assets)."),
            ("MIT", mit + " Non-commercial use only."),
            ("MIT", mit + " Commons Clause applies."),
            ("GPL", "GNU General Public License version 3"),
            ("GPL-3.0", "GNU Lesser General Public License version 3 with GNU General Public License references."),
        )
        for declared, text in cases:
            with self.subTest(declared=declared, text=text):
                self.assertFalse(matches(text, declared))
        self.assertTrue(matches("Redistribution and use in source and binary forms. Neither the name. All rights reserved.", "BSD-3-Clause"))

    def test_real_gpl_and_unlicense_texts_preserve_their_grants(self):
        matches = runpy.run_path(str(CHECKS / "rights-evidence.py"))["grant_text_matches"]
        fixtures = (
            ("gpl-2.0.txt", "GPL-2.0", "db296f2f7f35bca3a174efb0eb392b3b17bd94b341851429a3dff411b1c2fc73"),
            ("gpl-3.0.txt", "GPL-3.0", "c53a65c2fd561c87eaabf1072ef5dcab8653042bc15308465f52413585eb6271"),
            ("unlicense.txt", "Unlicense", "ca2abdf695884c77ea4b4a5b64ca7b732d9d9dbade4eebc1c2e76c53e9e3bc83"),
        )
        for filename, declared, digest in fixtures:
            content = (ROOT / "tests/fixtures/descriptors/licences" / filename).read_bytes()
            with self.subTest(declared=declared):
                self.assertEqual(sha(content), digest)
                self.assertTrue(matches(content.decode(), declared))
                if declared.startswith("GPL"):
                    self.assertFalse(matches(content.decode(), declared.replace("GPL", "LGPL")))

    def test_rebuild_reconsiders_only_verified_prior_legal_rejections(self):
        self.add_unrelated_resolution()
        resolution = self.record["resolution"]
        resolution["status"] = "unresolved"
        resolution["note"] = "Prior resolved citation retained."
        resolution["trail"] = [{"result": "WP7 correction: cited legal text does not state the recorded grant: example.jar"}]
        module = runpy.run_path(str(CHECKS / "rights-evidence.py"))
        policies = {("", "absent", "false"), ("MIT", "permissive", "false")}
        decision = module["resolve_grant"]("example.jar", self.record, ["https://github.com/unrelated/project"],
                                          self.jar.read_bytes(), self.cache, policies, True)
        self.assertEqual(decision, ("MIT", "permissive", "false"))
        self.assertEqual(resolution["status"], "resolved")
        self.assertIn("leading licence title", resolution["note"])

    def test_missing_commit_path_observation_is_refused(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project")
        self.commit_receipt_path.unlink()
        self.assert_no_permissive_grant("missing commit/path grant observation")

    def test_wrong_commit_path_observation_request_is_refused(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project")
        receipt = json.loads(self.commit_receipt_path.read_text())
        receipt["endpoint"] += "-different-commit"
        self.commit_receipt_path.write_text(json.dumps(receipt))
        self.assert_no_permissive_grant("commit/path grant observation request identity differs")

    def test_wrong_commit_path_observation_blob_is_refused(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project")
        citation = self.record["resolution"]["repository"]["licenceFile"]
        content = json.dumps({"type": "file", "path": "LICENSE", "sha": "0" * 40}).encode()
        (self.cache / sha(content)).write_bytes(content)
        receipt = citation["observation"]
        receipt["responseSha256"] = sha(content)
        self.commit_receipt_path.write_text(json.dumps(receipt))
        self.assert_no_permissive_grant("commit/path grant observation blob identity differs")

    def test_project_api_observation_must_match_acquisition_project(self):
        module = runpy.run_path(str(CHECKS / "rights-evidence.py"))
        content = json.dumps({"license": "", "description": "https://github.com/unrelated/project"}).encode()
        (self.cache.parent / "project-23.json").write_bytes(content)
        self.record["hostDeclaration"]["evidence"].append({
            "kind": "project-api", "source": "https://api.cfwidget.com/23",
            "sha256": sha(content), "observed": "",
        })
        with zipfile.ZipFile(self.jar) as archive:
            for project in ("17", "0"):
                with self.subTest(project=project), self.assertRaisesRegex(SystemExit, "does not belong to acquired artifact"):
                    module["observed_host"](archive, self.record["hostDeclaration"], "example.jar", self.cache, project)

    def test_rebuild_retains_unproven_attempt_as_unresolved(self):
        self.add_unrelated_resolution()
        module = runpy.run_path(str(CHECKS / "rights-evidence.py"))
        decision = module["resolve_grant"]("example.jar", self.record, [], self.jar.read_bytes(), self.cache,
                                          {("", "absent", "false")}, True)
        self.assertEqual(decision, ("", "absent", "false"))
        resolution = self.record["resolution"]
        self.assertEqual(resolution["status"], "unresolved")
        self.assertIn("prior resolved citation retained only as an attempted source", resolution["note"])
        self.assertEqual(resolution["repository"]["url"], "https://github.com/unrelated/project")

    def test_every_recorded_permission_and_total_is_reconciled(self):
        module = runpy.run_path(str(CHECKS / "rights-evidence.py"))
        self.record["surfaceEvidence"] = {"counts": {"textures": 1, "models": 0, "sounds": 0}}
        grants, acquisition = ["example.jar\tMIT\tpermissive\tfalse"], ["example.jar\tautomated-curseforge"]
        surfaces = ("code", "names", "behaviour", "branding", "textures", "models", "sounds")
        original = {"artifacts": [{"artifact": "example.jar", "rights": [{"surface": s} for s in surfaces]}]}
        expected = module["derived_rights"](original, grants, acquisition, self.evidence, {"example.jar": True})
        path = self.root / "rights.json"
        for field in (*surfaces, "sourceUse", "publication", "totals"):
            changed = json.loads(json.dumps(expected))
            if field in surfaces:
                next(r for r in changed["artifacts"][0]["rights"] if r["surface"] == field)["status"] = "forged"
            elif field == "totals":
                changed["totals"]["artifacts"] = 0
            else:
                changed["artifacts"][0][field] = "forged"
            path.write_text(json.dumps(changed))
            with self.subTest(field=field), self.assertRaisesRegex(SystemExit, "recorded (code permission|surface permissions)"):
                module["verify_recorded_rights"](path, grants, acquisition, self.evidence, {"example.jar": True})

    def test_provenance_summaries_are_regenerated_and_bound(self):
        module = runpy.run_path(str(CHECKS / "rights-evidence.py"))
        provenance = {
            "rights": [{"surface": "code", "status": "prohibited", "basis": "stale"}],
            "sources": [{"locator": "/rights-evidence.json", "use": "1 of the 1 jar-undeclared inputs resolved to a cited grant; 0 remain absent."}],
            "publication": {"notes": "permitted for 9 inputs under a cited permissive grant, conditional for 9, prohibited for 9 and unresolved for 9"},
        }
        records = {
            "artifacts": [{"rights": [{"surface": "code", "status": "cleared"}]}],
            "totals": {"sourceUse": {"permitted": 1, "conditional": 0, "prohibited": 0, "unresolved": 0}},
        }
        path = self.root / "provenance.json"
        path.write_text(json.dumps(provenance))
        reconcile = module["reconcile_provenance"]
        reconcile(path, records, self.evidence, True)
        updated = json.loads(path.read_text())
        self.assertEqual(updated["rights"][0]["status"], "cleared")
        self.assertIn("permitted for 1 inputs", updated["publication"]["notes"])
        self.assertIn("0 of the 1 jar-undeclared inputs", updated["sources"][0]["use"])
        for field in ("rights", "sources", "publication"):
            changed = json.loads(json.dumps(updated))
            if field == "rights":
                changed[field][0]["basis"] = "stale"
            elif field == "sources":
                changed[field][0]["use"] = changed[field][0]["use"].replace("0 of the 1", "1 of the 1")
            else:
                changed[field]["notes"] = changed[field]["notes"].replace("permitted for 1", "permitted for 9")
            path.write_text(json.dumps(changed))
            with self.subTest(field=field), self.assertRaisesRegex(SystemExit, "provenance rights summaries differ"):
                reconcile(path, records, self.evidence, False)

    def test_cited_grant_with_observed_source_link_permits_code(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project")
        result = self.run_binding()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("\tMIT\tpermissive\tfalse", (self.root / "grants.tsv").read_text())

    def test_cached_grant_bytes_mismatch_is_refused(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project")
        digest = self.record["resolution"]["repository"]["licenceFile"]["sha256"]
        (self.cache / digest).write_bytes(b"not the cited licence")
        self.assert_no_permissive_grant("cached grant SHA256 differs")

    def test_forged_host_declaration_is_refused(self):
        self.record["hostDeclaration"]["declared"] = "MIT"
        self.assert_no_permissive_grant("lacks independently observed declaration")

    def test_conflicting_host_and_resolution_are_refused(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project", "All Rights Reserved")
        self.record["hostDeclaration"]["declared"] = "All Rights Reserved"
        self.assert_no_permissive_grant("host declaration conflicts")

    def test_omitted_present_mcmod_info_is_refused(self):
        self.add_metadata("https://github.com/unrelated/project")
        self.record["hostDeclaration"]["evidence"].pop()
        self.assert_no_permissive_grant("incomplete host metadata observation")

    def test_wrong_jar_entry_prefix_is_refused(self):
        self.record["hostDeclaration"]["evidence"][0]["source"] = "other.jar!META-INF/MANIFEST.MF"
        self.assert_no_permissive_grant("host observation artifact identity differs")

    def test_later_licence_is_refused(self):
        self.add_unrelated_resolution()
        self.add_metadata("https://github.com/unrelated/project")
        self.record["resolution"]["repository"]["laterLicence"] = {"path": "LICENSE"}
        self.assert_no_permissive_grant("later licence is not a pinned host grant")

    def test_absent_host_grant_does_not_permit_code(self):
        result = self.run_binding()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("example.jar\t\tabsent\tfalse", (self.root / "grants.tsv").read_text())

    def test_wrong_host_observation_hash_is_refused(self):
        self.record["hostDeclaration"]["evidence"][0]["sha256"] = "0" * 64
        self.assert_no_permissive_grant("host observation differs")

    def test_artifact_identity_mismatch_does_not_permit_code(self):
        self.add_unrelated_resolution()
        self.record["artifactSha256"] = "0" * 64
        self.assert_no_permissive_grant("artifact SHA256 differs")

    def test_wrong_cited_grant_hash_does_not_permit_code(self):
        self.add_unrelated_resolution()
        self.record["resolution"]["repository"]["licenceFile"]["sha256"] = "0" * 64
        self.assert_no_permissive_grant("missing byte-bound grant cache")

    def test_cited_repository_identity_mismatch_does_not_permit_code(self):
        self.add_unrelated_resolution()
        self.record["resolution"]["repository"]["url"] = "https://github.com/different/owner"
        self.assert_no_permissive_grant("cited grant repository identity differs")

    def test_unrelated_grant_does_not_permit_code(self):
        self.add_unrelated_resolution()
        self.assert_no_permissive_grant("no byte-bound link to cited repository")

    def test_wrong_graph_identity_does_not_permit_code(self):
        self.add_unrelated_resolution()
        self.record["nodeIds"] = ["different-artifact"]
        self.assert_no_permissive_grant("artifact coverage or graph identity differs")


if __name__ == "__main__":
    unittest.main()
