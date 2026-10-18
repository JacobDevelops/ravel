"""Offline WP7 fixtures shared by the binding regression suites.

Child checks use a bounded timeout; no fixture performs network access.
"""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
CHECKS = ROOT / "registry/sevtech"
CHECK_TIMEOUT_SECONDS = 30


def sha(content: bytes, algorithm: str = "sha256") -> str:
    return hashlib.new(algorithm, content).hexdigest()


def jar_bytes() -> bytes:
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, "w") as archive:
        archive.writestr("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")
    return stream.getvalue()


class RightsEvidenceFixture(unittest.TestCase):
    def setUp(self) -> None:
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

    def run_binding(self) -> subprocess.CompletedProcess[str]:
        evidence = self.root / "evidence.json"
        evidence.write_text(json.dumps(self.evidence))
        return subprocess.run(
            [sys.executable, str(CHECKS / "rights-evidence.py"), str(evidence), str(self.paths),
             str(self.policies), str(self.root / "grants.tsv"), str(self.root / "acquisition.tsv"), str(self.cache)],
            capture_output=True, text=True, check=False,
            timeout=CHECK_TIMEOUT_SECONDS,
        )

    def assert_no_permissive_grant(self, message: str) -> None:
        result = self.run_binding()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(message, result.stderr)
        grants = self.root / "grants.tsv"
        self.assertTrue(
            result.returncode != 0 or not grants.exists() or "\tpermissive\t" not in grants.read_text(),
            "unbound citation granted code permission: " + result.stdout + result.stderr,
        )

    def add_unrelated_resolution(self) -> None:
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

    def add_commit_observation(self) -> None:
        repository = self.record["resolution"]["repository"]
        citation = repository["licenceFile"]
        endpoint = "https://api.github.com/repos/" + repository["url"].removeprefix("https://github.com/") + "/contents/" + citation["path"] + "?ref=" + repository["commit"]
        response = json.dumps({"type": "file", "path": citation["path"], "sha": citation["blobSha"]}).encode()
        (self.cache / sha(response)).write_bytes(response)
        observation = {"fetchedAt": "2026-10-19T06:05:25Z", "endpoint": endpoint, "responseSha256": sha(response)}
        self.commit_receipt_path = self.cache / ("github-contents-" + sha(endpoint.encode()) + ".json")
        self.commit_receipt_path.write_text(json.dumps(observation))
        citation["observation"] = observation

    def add_metadata(self, url: str, licence: str = "") -> None:
        content = json.dumps([{"modid": "example", "url": url, "license": licence}]).encode()
        with zipfile.ZipFile(self.jar, "w") as archive:
            archive.writestr("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")
            archive.writestr("mcmod.info", content)
        self.record["artifactSha256"] = sha(self.jar.read_bytes())
        self.record["hostDeclaration"]["evidence"].append({
            "kind": "jar-entry", "source": "example.jar!mcmod.info", "sha256": sha(content),
        })

    def add_exact_file_grant(self) -> None:
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
