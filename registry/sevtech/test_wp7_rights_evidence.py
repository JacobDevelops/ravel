"""Offline byte-bound publisher, repository and exact-file grant regressions."""
import hashlib
import json
import runpy
import unittest
import zipfile

from wp7_test_support import CHECKS, RightsEvidenceFixture, sha


class RightsEvidence(RightsEvidenceFixture):
    """Reject unbound rights evidence without contacting fixture URLs."""

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
